use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

use regex::Regex;

use super::{
    pdf_svg,
    process::{self, Job, RunError},
    tools::{find_tool, path_with_fallbacks},
};
use crate::files::hash_text;

const DRAWING_ENVIRONMENTS: [&str; 4] = ["tikzpicture", "tikzcd", "circuitikz", "pgfpicture"];
const VERBATIM_ENVIRONMENTS: [&str; 4] = ["verbatim", "lstlisting", "minted", "comment"];
const LATEX_ENGINES: [&str; 3] = ["pdflatex", "lualatex", "xelatex"];
const ERROR_LINE_LIMIT: usize = 160;
const TEX_FONT_PT: f64 = 10.0;

pub const TEX_MISSING: &str = "a TeX engine is needed to draw it (brew install tectonic)";

static BEGIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\begin\{([A-Za-z]+)\*?\}").unwrap());
static SVG_WIDTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<svg[^>]*?\swidth="([0-9.]+)(pt)?""#).unwrap());
static PAGE_LAYOUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\\usepackage(\[[^\]]*\])?\{(geometry|fancyhdr)\}|\\geometry\{|\\pagestyle\{)").unwrap());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawing {
    pub range: Range<usize>,
    pub body: String,
}

fn comment_start(line: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if c == '%' && !escaped {
            return Some(i);
        }
        escaped = c == '\\' && !escaped;
    }
    None
}

fn without_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        match comment_start(line) {
            Some(at) => {
                out.push_str(&line[..at]);
                out.push_str(&" ".repeat(line.len() - at - usize::from(line.ends_with('\n'))));
                if line.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => out.push_str(line),
        }
    }
    out
}

fn environment_end(text: &str, name: &str, from: usize) -> Option<usize> {
    let begin = format!("\\begin{{{name}}}");
    let end = format!("\\end{{{name}}}");
    let mut depth = 1;
    let mut at = from;
    loop {
        let next_end = text[at..].find(&end)? + at;
        let next_begin = text[at..].find(&begin).map(|i| i + at);
        match next_begin {
            Some(b) if b < next_end => {
                depth += 1;
                at = b + begin.len();
            }
            _ => {
                depth -= 1;
                at = next_end + end.len();
                if depth == 0 {
                    return Some(at);
                }
            }
        }
    }
}

pub fn find_drawings(source: &str) -> Vec<Drawing> {
    let visible = without_comments(source);
    let mut drawings = Vec::new();
    let mut at = 0;
    while let Some(found) = BEGIN.captures(&visible[at..]) {
        let whole = found.get(0).expect("a match has a whole range");
        let start = at + whole.start();
        let name = found[1].to_owned();
        let body_start = at + whole.end();
        if VERBATIM_ENVIRONMENTS.contains(&name.as_str()) {
            at = environment_end(&visible, &name, body_start).unwrap_or(visible.len());
            continue;
        }
        if DRAWING_ENVIRONMENTS.contains(&name.as_str()) {
            let Some(end) = environment_end(&visible, &name, body_start) else { break };
            drawings.push(Drawing { range: start..end, body: source[start..end].to_owned() });
            at = end;
            continue;
        }
        at = body_start;
    }
    drawings
}

pub fn preamble(source: &str) -> String {
    let Some(body) = source.find("\\begin{document}") else { return String::new() };
    source[..body]
        .lines()
        .filter(|line| !line.trim_start().starts_with("\\documentclass") && !PAGE_LAYOUT.is_match(line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn standalone(preamble: &str, drawing: &str) -> String {
    let needs_tikz = !preamble.contains("tikz") && !preamble.contains("pgf");
    format!(
        "\\documentclass[border=2pt]{{standalone}}\n{}{preamble}\n\\begin{{document}}\n{drawing}\n\\end{{document}}\n",
        if needs_tikz { "\\usepackage{tikz}\n" } else { "" },
    )
}

pub fn replace_drawings(source: &str, drawings: &[Drawing], replacements: &[String]) -> String {
    let mut out = String::with_capacity(source.len());
    let mut copied = 0;
    for (drawing, replacement) in drawings.iter().zip(replacements) {
        out.push_str(&source[copied..drawing.range.start]);
        out.push_str(replacement);
        out.push_str(&"\n".repeat(source[drawing.range.clone()].matches('\n').count()));
        copied = drawing.range.end;
    }
    out.push_str(&source[copied..]);
    out
}

pub fn width_in_em(svg: &str) -> Option<f64> {
    let points: f64 = SVG_WIDTH.captures(svg)?[1].parse().ok()?;
    (points > 0.0).then(|| (points / TEX_FONT_PT * 100.0).round() / 100.0)
}

pub fn image_for(svg: &Path, width_em: Option<f64>) -> String {
    match width_em {
        Some(em) => format!("\\includegraphics[width={em}em]{{{}}}", svg.display()),
        None => format!("\\includegraphics{{{}}}", svg.display()),
    }
}

pub fn note_for(problem: &str) -> String {
    let safe: String = problem
        .chars()
        .map(|c| if c.is_alphanumeric() || " .,:;()-/'".contains(c) { c } else { ' ' })
        .collect();
    format!("\\emph{{[Drawing not shown: {}]}}", safe.split_whitespace().collect::<Vec<_>>().join(" "))
}

pub trait TexCompiler: Sync {
    fn compile(&self, tex: &str, cwd: &Path, job_key: &Path) -> Result<Vec<u8>, String>;
}

pub struct SystemTex;

impl TexCompiler for SystemTex {
    fn compile(&self, tex: &str, cwd: &Path, job_key: &Path) -> Result<Vec<u8>, String> {
        let dir = std::env::temp_dir().join(format!("washi-tikz-build-{}-{}", std::process::id(), hash_text(tex)));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join("drawing.tex");
        fs::write(&file, tex).map_err(|e| e.to_string())?;
        let result = compile_with_engine(&file, &dir, cwd, job_key);
        let _ = fs::remove_dir_all(&dir);
        result
    }
}

fn compile_with_engine(file: &Path, out_dir: &Path, cwd: &Path, job_key: &Path) -> Result<Vec<u8>, String> {
    let mut command = if let Some(engine) = LATEX_ENGINES.iter().find_map(|e| find_tool(e)) {
        let mut c = Command::new(engine);
        c.arg("-interaction=nonstopmode")
            .arg("-halt-on-error")
            .arg(format!("-output-directory={}", out_dir.display()))
            .arg(file);
        c
    } else if let Some(tectonic) = find_tool("tectonic") {
        let mut c = Command::new(tectonic);
        c.arg("--outdir").arg(out_dir).arg(file);
        c
    } else {
        return Err(TEX_MISSING.to_owned());
    };
    command.current_dir(cwd).env("PATH", path_with_fallbacks());
    let key = PathBuf::from(format!("{}.drawings", job_key.display()));
    let job = Job::start(&key);
    let output = process::run(&mut command, process::timeout_from_env(), job.cancelled()).map_err(|e| match e {
        RunError::Spawn(e) => format!("cannot start the TeX engine: {e}"),
        RunError::TimedOut(limit) => format!("the TeX engine was stopped after {} seconds", limit.as_secs()),
        RunError::Cancelled => "stopped because a newer render replaced it".to_owned(),
    })?;
    let pdf = out_dir.join("drawing.pdf");
    if output.status.success() || pdf.is_file() {
        if let Ok(bytes) = fs::read(&pdf) {
            return Ok(bytes);
        }
    }
    let log = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    Err(first_error(&log).unwrap_or_else(|| "the TeX engine failed".to_owned()))
}

fn first_error(log: &str) -> Option<String> {
    log.lines()
        .find(|l| l.starts_with('!') || l.starts_with("error:"))
        .map(|l| l.trim_start_matches(['!', ' ']).trim_start_matches("error: ").chars().take(ERROR_LINE_LIMIT).collect())
}

pub struct Drawings<'a, T: TexCompiler> {
    pub tex: &'a T,
    pub cache: PathBuf,
}

pub fn cache_dir() -> PathBuf {
    std::env::temp_dir().join("washi-tikz-cache")
}

impl<T: TexCompiler> Drawings<'_, T> {
    pub fn prepare(&self, source: &str, cwd: &Path, job_key: &Path) -> String {
        let drawings = find_drawings(source);
        if drawings.is_empty() {
            return source.to_owned();
        }
        let preamble = preamble(source);
        let replacements: Vec<String> = drawings
            .iter()
            .map(|drawing| match self.svg_for(&preamble, &drawing.body, cwd, job_key) {
                Ok(svg) => image_for(&svg, fs::read_to_string(&svg).ok().as_deref().and_then(width_in_em)),
                Err(problem) => note_for(&problem),
            })
            .collect();
        replace_drawings(source, &drawings, &replacements)
    }

    fn svg_for(&self, preamble: &str, body: &str, cwd: &Path, job_key: &Path) -> Result<PathBuf, String> {
        let tex = standalone(preamble, body);
        let cached = self.cache.join(format!("{}.svg", hash_text(&format!("{}\n{tex}", cwd.display()))));
        if cached.is_file() {
            return Ok(cached);
        }
        let pdf = self.tex.compile(&tex, cwd, job_key)?;
        let svg = pdf_svg::first_page_to_svg(pdf)?;
        fs::create_dir_all(&self.cache).map_err(|e| e.to_string())?;
        fs::write(&cached, svg).map_err(|e| e.to_string())?;
        Ok(cached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const DOCUMENT: &str = "\\documentclass{article}\n\\usepackage[margin=2cm]{geometry}\n\\usepackage{tikz}\n\\newcommand{\\R}{\\mathbb{R}}\n\\begin{document}\nBefore.\n\\begin{figure}\n\\centering\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}\n\\caption{A line}\n\\end{figure}\n% \\begin{tikzpicture} commented out \\end{tikzpicture}\n\\begin{verbatim}\n\\begin{tikzpicture}\\end{tikzpicture}\n\\end{verbatim}\nAfter.\n\\end{document}\n";

    struct Fake {
        result: Result<Vec<u8>, String>,
        calls: Mutex<Vec<String>>,
    }

    impl TexCompiler for Fake {
        fn compile(&self, tex: &str, _cwd: &Path, _job_key: &Path) -> Result<Vec<u8>, String> {
            self.calls.lock().unwrap().push(tex.to_owned());
            self.result.clone()
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-tikz-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_drawings_but_not_in_comments_or_verbatim() {
        let found = find_drawings(DOCUMENT);
        assert_eq!(found.len(), 1);
        assert!(found[0].body.starts_with("\\begin{tikzpicture}") && found[0].body.ends_with("\\end{tikzpicture}"));
    }

    #[test]
    fn nested_environments_of_the_same_name_end_at_the_right_place() {
        let source = "\\begin{document}\n\\begin{tikzpicture}\n\\node {\\begin{tikzpicture}\\end{tikzpicture}};\n\\end{tikzpicture}\ntail\n";
        let found = find_drawings(source);
        assert_eq!(found.len(), 1);
        assert!(found[0].body.ends_with("};\n\\end{tikzpicture}"), "{}", found[0].body);
    }

    #[test]
    fn other_drawing_environments_are_found_too() {
        let source = "\\begin{document}\n\\begin{tikzcd} A \\arrow[r] & B \\end{tikzcd}\n\\begin{circuitikz}\\draw (0,0) to[R] (2,0);\\end{circuitikz}\n";
        assert_eq!(find_drawings(source).len(), 2);
    }

    #[test]
    fn the_preamble_drops_the_class_and_page_layout_but_keeps_packages_and_macros() {
        let pre = preamble(DOCUMENT);
        assert!(!pre.contains("documentclass") && !pre.contains("geometry"), "{pre}");
        assert!(pre.contains("\\usepackage{tikz}") && pre.contains("\\newcommand{\\R}"), "{pre}");
    }

    #[test]
    fn a_standalone_document_loads_tikz_only_when_the_preamble_does_not() {
        let with = standalone("\\usepackage{tikz}", "\\begin{tikzpicture}\\end{tikzpicture}");
        assert!(with.starts_with("\\documentclass[border=2pt]{standalone}"));
        assert_eq!(with.matches("\\usepackage{tikz}").count(), 1);
        let without = standalone("", "\\begin{tikzpicture}\\end{tikzpicture}");
        assert!(without.contains("\\usepackage{tikz}"));
        assert!(without.contains("\\begin{document}\n\\begin{tikzpicture}"));
    }

    #[test]
    fn replacing_keeps_every_line_in_place() {
        let found = find_drawings(DOCUMENT);
        let out = replace_drawings(DOCUMENT, &found, &["\\includegraphics{x.svg}".to_owned()]);
        assert_eq!(out.lines().count(), DOCUMENT.lines().count());
        assert!(out.contains("\\centering\n\\includegraphics{x.svg}\n\n\n\\caption{A line}"), "{out}");
    }

    #[test]
    fn a_compiled_drawing_becomes_a_cached_svg_image() {
        let dir = scratch("cache");
        let fake = Fake { result: Ok(pdf_svg::tests::tiny_pdf()), calls: Mutex::new(Vec::new()) };
        let drawings = Drawings { tex: &fake, cache: dir.join("cache") };
        let first = drawings.prepare(DOCUMENT, &dir, &dir.join("a.tex"));
        let second = drawings.prepare(DOCUMENT, &dir, &dir.join("a.tex"));
        assert_eq!(fake.calls.lock().unwrap().len(), 1, "the second render uses the cache");
        assert_eq!(first, second);
        let path = first.split("\\includegraphics[width=").nth(1).unwrap().split('{').nth(1).unwrap().split('}').next().unwrap();
        assert!(fs::read_to_string(path).unwrap().starts_with("<svg"));
        assert!(fake.calls.lock().unwrap()[0].contains("\\draw (0,0) -- (1,1);"));
    }

    #[test]
    fn a_failed_or_impossible_drawing_leaves_a_visible_note() {
        let dir = scratch("note");
        let fake = Fake { result: Err(TEX_MISSING.to_owned()), calls: Mutex::new(Vec::new()) };
        let drawings = Drawings { tex: &fake, cache: dir.join("cache") };
        let out = drawings.prepare(DOCUMENT, &dir, &dir.join("a.tex"));
        assert!(out.contains("\\emph{[Drawing not shown: a TeX engine is needed to draw it (brew install tectonic)]}"), "{out}");
        assert_eq!(out.lines().count(), DOCUMENT.lines().count());
    }

    #[test]
    fn a_document_without_drawings_is_untouched_and_nothing_runs() {
        let dir = scratch("none");
        let fake = Fake { result: Err("unused".into()), calls: Mutex::new(Vec::new()) };
        let drawings = Drawings { tex: &fake, cache: dir.join("cache") };
        let source = "\\begin{document}\nPlain.\n\\end{document}\n";
        assert_eq!(drawings.prepare(source, &dir, &dir.join("a.tex")), source);
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_drawing_is_sized_like_its_ten_point_text() {
        assert_eq!(width_in_em("<svg viewBox=\"0 0 208.48 24.23\" width=\"208.48\" height=\"24.23\">"), Some(20.85));
        assert_eq!(width_in_em("<svg width=\"50pt\">"), Some(5.0));
        assert_eq!(width_in_em("<svg viewBox=\"0 0 1 1\">"), None);
        assert_eq!(image_for(Path::new("/c/a.svg"), Some(20.85)), "\\includegraphics[width=20.85em]{/c/a.svg}");
        assert_eq!(image_for(Path::new("/c/a.svg"), None), "\\includegraphics{/c/a.svg}");
    }

    #[test]
    fn notes_cannot_inject_latex() {
        assert_eq!(note_for("bad \\input{x} $y$ {z}"), "\\emph{[Drawing not shown: bad input x y z ]}".replace(" ]", "]"));
    }

    #[test]
    fn the_first_tex_error_is_reported() {
        assert_eq!(first_error("noise\n! Undefined control sequence.\nl.3"), Some("Undefined control sequence.".to_owned()));
        assert_eq!(first_error("error: drawing.tex:3: Undefined control sequence"), Some("drawing.tex:3: Undefined control sequence".to_owned()));
        assert_eq!(first_error("all fine"), None);
    }

    #[test]
    #[ignore = "runs a real TeX engine"]
    fn a_real_engine_draws_a_tikz_picture() {
        let dir = scratch("real");
        let drawings = Drawings { tex: &SystemTex, cache: dir.join("cache") };
        let out = drawings.prepare(DOCUMENT, &dir, &dir.join("a.tex"));
        let path = out.split("\\includegraphics[width=").nth(1).unwrap_or_else(|| panic!("{out}")).split('{').nth(1).unwrap().split('}').next().unwrap();
        assert!(fs::read_to_string(path).unwrap().contains("<path"));
    }
}
