use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock,
    },
};

use regex::Regex;

use super::{
    process::{self, Job, RunError},
    tools::{find_tool, path_with_fallbacks},
    latex_equations, latex_marks,
    tikz::{self, Drawings, TexCompiler},
    Output, Rendered, Renderer,
};

const ROOT_SCAN_LINES: usize = 20;
const ROOT_SCAN_BYTES: u64 = 4096;
const TEMPLATE: &str = "$if(title)$<h1 class=\"title\">$title$</h1>\n$endif$$if(author)$<p class=\"author\">$for(author)$$author$$sep$, $endfor$</p>\n$endif$$if(date)$<p class=\"date\">$date$</p>\n$endif$$if(abstract)$<section class=\"abstract\"><h2>Abstract</h2>\n$abstract$\n</section>\n$endif$$body$\n";
const STDERR_LIMIT: usize = 2000;

pub const PANDOC_MISSING: &str =
    "LaTeX is shown with pandoc, which was not found. Install it with `brew install pandoc`.";

static NUMBERING_CONTROL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\(notag|nonumber)\b").unwrap());
static ROOT_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*%\s*!\s*tex\s+root\s*=\s*(.+?)\s*$").unwrap());

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn magic_root(source: &Path, text: &str) -> Option<PathBuf> {
    let named = text
        .lines()
        .take(ROOT_SCAN_LINES)
        .find_map(|line| ROOT_COMMENT.captures(line).map(|c| c[1].trim_matches('"').to_owned()))?;
    let mut root = source.parent().unwrap_or(Path::new(".")).join(named);
    if root.extension().is_none() {
        root.set_extension("tex");
    }
    let root = normalize(&root);
    (root.is_file() && root != normalize(source)).then_some(root)
}

fn effective_source(path: &Path) -> PathBuf {
    let mut head = Vec::new();
    let read = fs::File::open(path).and_then(|f| f.take(ROOT_SCAN_BYTES).read_to_end(&mut head));
    if read.is_err() {
        return path.to_path_buf();
    }
    magic_root(path, &String::from_utf8_lossy(&head)).unwrap_or_else(|| path.to_path_buf())
}

fn folder_of(path: &Path) -> &Path {
    path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

pub trait HtmlConverter: Sync {
    fn convert(&self, input: &Path, cwd: &Path, job_key: &Path) -> Result<String, String>;
}

pub struct LatexRenderer<C, T> {
    pub converter: C,
    pub tex: T,
}

impl<C: HtmlConverter, T: TexCompiler> Renderer for LatexRenderer<C, T> {
    fn name(&self) -> &'static str {
        "latex"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tex", "latex"]
    }

    fn render(&self, path: &Path) -> Result<Output, String> {
        let root = effective_source(path);
        let text = fs::read_to_string(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        self.html(&text, stem_of(&root), folder_of(&root), path).map(Output::Html)
    }

    fn render_text(&self, source: &str) -> Result<Output, String> {
        self.html(source, "pasted", &std::env::temp_dir(), Path::new("pasted.tex")).map(Output::Html)
    }

    fn dependency_dirs(&self, path: &Path) -> Vec<PathBuf> {
        super::deps::latex(&effective_source(path))
    }

    fn render_buffer(&self, path: &Path, text: &str) -> Rendered {
        let output = match magic_root(path, text) {
            Some(root) => fs::read_to_string(&root)
                .map_err(|e| format!("{}: {e}", root.display()))
                .and_then(|disk| self.html(&disk, stem_of(&root), folder_of(&root), path)),
            None => self.html(text, stem_of(path), folder_of(path), path),
        };
        Rendered { output: output.map(Output::Html), diagnostics: Vec::new() }
    }

    fn buffer_dependency_dirs(&self, path: &Path, text: &str) -> Vec<PathBuf> {
        match magic_root(path, text) {
            Some(root) => super::deps::latex(&root),
            None => super::deps::latex_text(path, Some(text)),
        }
    }
}

impl<C: HtmlConverter, T: TexCompiler> LatexRenderer<C, T> {
    fn html(&self, text: &str, stem: &str, cwd: &Path, job_key: &Path) -> Result<String, String> {
        let drawings = Drawings { tex: &self.tex, cache: tikz::cache_dir() };
        let numbering = latex_equations::number(text);
        let prepared = without_numbering_controls(&drawings.prepare(text, cwd, job_key));
        let marked = latex_marks::inject(&prepared);
        let attempt = Scratch::new(stem, &marked).and_then(|scratch| self.converter.convert(&scratch.input, cwd, job_key));
        let html = match attempt {
            Ok(html) => latex_marks::read_back(&html),
            Err(message) if marked != prepared && message.starts_with("pandoc failed") => {
                let plain = Scratch::new(stem, &prepared)?;
                self.converter.convert(&plain.input, cwd, job_key)?
            }
            Err(message) => return Err(message),
        };
        Ok(latex_equations::apply(&html, &numbering))
    }
}

fn without_numbering_controls(text: &str) -> String {
    NUMBERING_CONTROL.replace_all(text, "").into_owned()
}

fn stem_of(path: &Path) -> &str {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("document")
}

struct Scratch {
    dir: PathBuf,
    input: PathBuf,
}

impl Scratch {
    fn new(stem: &str, text: &str) -> Result<Self, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "washi-latex-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let input = dir.join(format!("{stem}.tex"));
        fs::write(&input, text).map_err(|e| e.to_string())?;
        Ok(Self { dir, input })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub struct PandocConverter;

impl HtmlConverter for PandocConverter {
    fn convert(&self, input: &Path, cwd: &Path, job_key: &Path) -> Result<String, String> {
        let pandoc = find_tool("pandoc").ok_or(PANDOC_MISSING)?;
        let template = Scratch::new("template", "")?;
        let template_file = template.dir.join("body.html");
        fs::write(&template_file, TEMPLATE).map_err(|e| e.to_string())?;

        let mut command = Command::new(pandoc);
        command
            .args(pandoc_args(input, cwd, &template_file))
            .current_dir(cwd)
            .env("PATH", path_with_fallbacks());
        let job = Job::start(job_key);
        let output = process::run(&mut command, process::timeout_from_env(), job.cancelled()).map_err(|e| match e {
            RunError::Spawn(e) => format!("cannot start pandoc: {e}"),
            RunError::TimedOut(limit) => format!(
                "pandoc was stopped after {} seconds; set {} to change the limit",
                limit.as_secs(),
                process::TIMEOUT_ENV
            ),
            RunError::Cancelled => "stopped because a newer render replaced it".to_string(),
        })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        Err(describe_failure(&String::from_utf8_lossy(&output.stderr)))
    }
}

fn pandoc_args(input: &Path, cwd: &Path, template: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-f",
        "latex",
        "-t",
        "html5",
        "--math-method=mathml",
        "--citeproc",
        "--wrap=none",
        "--standalone",
        "--embed-resources",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push("--template".into());
    args.push(template.into());
    args.push("--resource-path".into());
    args.push(cwd.into());
    args.push(input.into());
    args
}

fn describe_failure(stderr: &str) -> String {
    let trimmed = stderr.trim();
    let shown: String = trimmed.chars().take(STDERR_LIMIT).collect();
    if shown.is_empty() {
        "pandoc failed without a message".to_string()
    } else {
        format!("pandoc failed:\n{shown}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Fake {
        html: Result<String, String>,
        seen: Mutex<Vec<(PathBuf, PathBuf, String)>>,
    }

    impl Fake {
        fn ok(html: &str) -> Self {
            Self { html: Ok(html.to_owned()), seen: Mutex::new(Vec::new()) }
        }

        fn failing(message: &str) -> Self {
            Self { html: Err(message.to_owned()), seen: Mutex::new(Vec::new()) }
        }
    }

    impl HtmlConverter for Fake {
        fn convert(&self, input: &Path, cwd: &Path, _job_key: &Path) -> Result<String, String> {
            let text = fs::read_to_string(input).unwrap_or_default();
            self.seen.lock().unwrap().push((input.to_path_buf(), cwd.to_path_buf(), text));
            self.html.clone()
        }
    }

    struct NoTex;

    impl TexCompiler for NoTex {
        fn compile(&self, _tex: &str, _cwd: &Path, _job_key: &Path) -> Result<Vec<u8>, String> {
            Err(tikz::TEX_MISSING.to_owned())
        }
    }

    fn renderer(converter: Fake) -> LatexRenderer<Fake, NoTex> {
        LatexRenderer { converter, tex: NoTex }
    }

    #[test]
    fn a_tikz_picture_is_replaced_before_pandoc_sees_it_without_moving_lines() {
        let dir = project("tikz");
        let file = dir.join("paper.tex");
        let text = "\\documentclass{article}\n\\begin{document}\nBefore.\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}\nAfter.\n\\end{document}\n";
        fs::write(&file, text).unwrap();
        let renderer = renderer(Fake::ok("<p>x</p>"));
        renderer.render(&file).unwrap();
        let seen = renderer.converter.seen.lock().unwrap();
        assert!(!seen[0].2.contains("tikzpicture"), "{}", seen[0].2);
        assert!(seen[0].2.contains("Drawing not shown"), "{}", seen[0].2);
        assert_eq!(seen[0].2.lines().count(), text.lines().count());
        fs::remove_dir_all(dir).ok();
    }

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-latex-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("chapters")).unwrap();
        dir
    }

    fn multi_file_project(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = project(name);
        let main = dir.join("main.tex");
        let chapter = dir.join("chapters/one.tex");
        fs::write(&main, "\\documentclass{article}\\begin{document}\\input{chapters/one}\\end{document}").unwrap();
        fs::write(&chapter, "% !TEX root = ../main.tex\n\\section{One}").unwrap();
        (dir, main, chapter)
    }

    #[test]
    fn the_root_magic_comment_names_the_main_file() {
        let (dir, main, chapter) = multi_file_project("root-comment");
        assert_eq!(magic_root(&chapter, "% !TEX root = ../main.tex\n"), Some(main.clone()));
        assert_eq!(magic_root(&chapter, "%!TeX root=../main\nbody"), Some(main.clone()), "no spaces, mixed case, no extension");
        assert_eq!(magic_root(&chapter, "% !TEX root = \"../main.tex\""), Some(main.clone()), "quotes are ignored");
        assert_eq!(magic_root(&chapter, "% !TEX root = ../gone.tex"), None, "a missing file is ignored");
        assert_eq!(magic_root(&main, "% !TEX root = main.tex"), None, "a file is never its own root");
        assert_eq!(magic_root(&chapter, "\\section{No comment}"), None);
        let late = format!("{}% !TEX root = ../main.tex", "\n".repeat(25));
        assert_eq!(magic_root(&chapter, &late), None, "only the first lines are read");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_saved_file_is_converted_from_its_own_folder() {
        let dir = project("saved");
        let file = dir.join("paper.tex");
        fs::write(&file, "\\section{A}").unwrap();
        let renderer = renderer(Fake::ok("<h2>A</h2>"));
        assert!(matches!(renderer.render(&file), Ok(Output::Html(h)) if h == "<h2>A</h2>"));
        let seen = renderer.converter.seen.lock().unwrap();
        assert_eq!(seen[0].1, dir, "converted from the documents own folder");
        assert!(seen[0].2.contains("\\section{A}"));
        assert!(!seen[0].0.exists(), "the copy is removed after the conversion");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_chapter_with_a_root_comment_is_converted_through_the_main_file() {
        let (dir, main, chapter) = multi_file_project("through-root");
        let renderer = renderer(Fake::ok("<p>x</p>"));
        renderer.render(&chapter).unwrap();
        let seen = renderer.converter.seen.lock().unwrap();
        assert_eq!(seen[0].0.file_name(), main.file_name());
        assert!(seen[0].2.contains("\\input{chapters/one}"), "the main file's text is converted");
        assert_eq!(seen[0].1, dir);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn an_unsaved_buffer_is_converted_from_a_copy_next_to_the_documents_includes() {
        let dir = project("buffer");
        let file = dir.join("paper.tex");
        let renderer = renderer(Fake::ok("<p>b</p>"));
        let rendered = renderer.render_buffer(&file, "\\section{Unsaved}");
        assert!(matches!(rendered.output, Ok(Output::Html(_))));
        assert!(rendered.diagnostics.is_empty());
        let seen = renderer.converter.seen.lock().unwrap();
        assert_eq!(seen[0].2, "\\section{Unsaved}");
        assert_eq!(seen[0].1, dir, "relative \\input paths must resolve from the documents folder");
        assert!(!seen[0].0.exists(), "the copy is removed after the conversion");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_buffer_with_a_root_comment_converts_the_main_file() {
        let (dir, main, chapter) = multi_file_project("buffer-root");
        let renderer = renderer(Fake::ok("<p>x</p>"));
        renderer.render_buffer(&chapter, "% !TEX root = ../main.tex\n\\section{Edited}");
        let seen = renderer.converter.seen.lock().unwrap();
        assert_eq!(seen[0].0.file_name(), main.file_name());
        assert!(seen[0].2.contains("\\input{chapters/one}"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn pasted_text_is_converted_from_a_temporary_file_that_is_removed_afterwards() {
        let renderer = renderer(Fake::ok("<p>p</p>"));
        assert!(matches!(renderer.render_text("\\section{Pasted}"), Ok(Output::Html(_))));
        let seen = renderer.converter.seen.lock().unwrap();
        assert_eq!(seen[0].2, "\\section{Pasted}");
        assert!(!seen[0].0.exists());
    }

    #[test]
    fn a_failed_conversion_is_reported_as_an_error() {
        let dir = project("failed");
        let file = dir.join("paper.tex");
        fs::write(&file, "x").unwrap();
        let renderer = renderer(Fake::failing("pandoc failed:\nbad"));
        assert_eq!(renderer.render(&file).err().as_deref(), Some("pandoc failed:\nbad"));
        let rendered = renderer.render_buffer(&file, "x");
        assert!(rendered.output.is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn numbering_controls_that_pandoc_cannot_read_are_dropped() {
        let text = "\\begin{align}\n a &= b \\\\\n &= c \\notag\n d &= e \\nonumber\n\\end{align}\n\\notable";
        let out = without_numbering_controls(text);
        assert!(!out.contains("\\notag") && !out.contains("\\nonumber"), "{out}");
        assert!(out.contains("\\notable"), "only whole commands are removed: {out}");
        assert_eq!(out.lines().count(), text.lines().count());
    }

    #[test]
    fn the_missing_tool_message_says_how_to_install_pandoc() {
        assert!(PANDOC_MISSING.contains("brew install pandoc"));
    }

    #[test]
    fn pandoc_is_asked_for_a_body_only_html_with_embedded_images_and_citations() {
        let args = pandoc_args(Path::new("/d/a.tex"), Path::new("/d"), Path::new("/t/body.html"));
        let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        for flag in ["--math-method=mathml", "--citeproc", "--standalone", "--embed-resources", "--wrap=none"] {
            assert!(args.iter().any(|a| a == flag), "{flag} missing from {args:?}");
        }
        let after = |name: &str| args[args.iter().position(|a| a == name).unwrap() + 1].clone();
        assert_eq!(after("--template"), "/t/body.html");
        assert_eq!(after("--resource-path"), "/d");
        assert_eq!(args.last().unwrap(), "/d/a.tex");
    }

    #[test]
    fn long_pandoc_errors_are_shortened_and_empty_ones_still_say_something() {
        assert!(describe_failure(&"x".repeat(5000)).chars().count() < 2100);
        assert_eq!(describe_failure("  \n"), "pandoc failed without a message");
        assert!(describe_failure("Error at line 3").starts_with("pandoc failed:\nError at line 3"));
    }

    #[test]
    fn dependency_dirs_follow_the_root_file() {
        let (dir, main, chapter) = multi_file_project("deps");
        let renderer = renderer(Fake::ok(""));
        assert_eq!(renderer.dependency_dirs(&chapter), super::super::deps::latex(&main));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    #[ignore = "runs the real pandoc"]
    fn real_pandoc_converts_the_example_paper() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/paper.tex");
        let Output::Html(html) = LatexRenderer { converter: PandocConverter, tex: tikz::SystemTex }.render(&path).expect("pandoc converts the example");
        for needle in ["<math", "Definition", "<table", "Knuth", "Incremental Rendering", "We describe a viewer", "kiwamizamurai", "data:image/svg+xml", "The render pipeline", "<mtable"] {
            assert!(html.contains(needle), "paper.tex has no {needle}");
        }
        assert!(!html.contains("$$\\begin{align}"), "the align environment is converted to MathML, not left as raw TeX");
        assert!(html.contains("class=\"eq-number\""), "the numbered equation shows its number");
        assert!(!html.contains("[eq:total]"), "the reference shows the number, not the label");
        assert!(!html.contains("<html"), "only the body is returned");
        assert!(html.contains("data-sourcepos=\""), "paragraphs carry their source line");
        assert!(!html.contains("washi-L"), "no line marker is left in the output");
    }
}
