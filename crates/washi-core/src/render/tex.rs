use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

use regex::Regex;

use super::{
    process::{self, Job, RunError},
    synctex::SyncTex,
    tools::find_tool,
    Output, PreviewPosition, Rendered, Renderer, SourceLocation,
};

const MIRROR_PREFIX: &str = ".washi-buf-";
const STALE_MIRROR_AGE: std::time::Duration = std::time::Duration::from_secs(60 * 60);
const ROOT_SCAN_LINES: usize = 20;
const ROOT_SCAN_BYTES: u64 = 4096;

static ROOT_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*%\s*!\s*tex\s+root\s*=\s*(.+?)\s*$").unwrap());
static BIBLATEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\(?:usepackage|RequirePackage)\s*(?:\[([^\]]*)\])?\s*\{[^}]*\bbiblatex\b[^}]*\}").unwrap()
});
static BIBTEX_BACKEND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)backend\s*=\s*bibtex").unwrap());

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

pub trait TexEngine: Sync {
    fn compile(&self, source: &Path, out_dir: &Path, cwd: &Path) -> Result<(), String>;
}

pub struct TexRenderer<E>(pub E);

impl<E: TexEngine> Renderer for TexRenderer<E> {
    fn name(&self) -> &'static str {
        "latex"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tex", "latex"]
    }

    fn render(&self, path: &Path) -> Result<Output, String> {
        self.build(&effective_source(path))
    }

    fn render_text(&self, source: &str) -> Result<Output, String> {
        let dir = std::env::temp_dir().join(format!("washi-paste-{}", std::process::id()));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join("pasted.tex");
        fs::write(&file, source).map_err(|e| e.to_string())?;
        self.render(&file)
    }

    fn dependency_dirs(&self, path: &Path) -> Vec<PathBuf> {
        super::deps::latex(&effective_source(path))
    }

    fn render_buffer(&self, path: &Path, text: &str) -> Rendered {
        let output = match magic_root(path, text) {
            Some(root) => self.render_through_root(path, &root, text),
            None => self.render_mirror(path, text),
        };
        Rendered { output, diagnostics: Vec::new() }
    }

    fn buffer_dependency_dirs(&self, path: &Path, text: &str) -> Vec<PathBuf> {
        match magic_root(path, text) {
            Some(root) => super::deps::latex(&root),
            None => super::deps::latex_text(path, Some(text)),
        }
    }

    fn locate_forward(&self, path: &Path, line: u32, _column: u32) -> Result<Option<PreviewPosition>, String> {
        let root = effective_source(path);
        let synctex = read_synctex(&root)?;
        Ok(synctex
            .forward(|input| same_file_from(&root, path, input), line)
            .map(|hit| PreviewPosition { page: hit.page, x: hit.x, y: hit.y }))
    }

    fn locate(&self, path: &Path, page: usize, x: f64, y: f64) -> Result<Option<SourceLocation>, String> {
        let root = effective_source(path);
        let synctex = read_synctex(&root)?;
        let Some(hit) = synctex.inverse(page as u32, x, y) else {
            return Ok(None);
        };
        let file = resolve_input(&root, &hit.input);
        Ok(Some(SourceLocation { file, line: hit.line as usize, column: 1 }))
    }
}

impl<E: TexEngine> TexRenderer<E> {
    fn build(&self, source: &Path) -> Result<Output, String> {
        let stem = source.file_stem().and_then(|s| s.to_str()).ok_or("invalid file name")?;
        let out_dir = out_dir_for(source);
        fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;

        self.0.compile(source, &out_dir, source.parent().unwrap_or(Path::new(".")))?;

        fs::read(out_dir.join(format!("{stem}.pdf")))
            .map(Output::Pdf)
            .map_err(|e| format!("cannot read the PDF: {e}"))
    }
    fn render_through_root(&self, path: &Path, root: &Path, text: &str) -> Result<Output, String> {
        let unsaved = fs::read_to_string(path).map(|disk| disk != text).unwrap_or(true);
        if unsaved {
            let stem = root.file_stem().and_then(|s| s.to_str()).ok_or("invalid file name")?;
            if let Ok(bytes) = fs::read(out_dir_for(root).join(format!("{stem}.pdf"))) {
                return Ok(Output::Pdf(bytes));
            }
        }
        self.build(root)
    }

    fn render_mirror(&self, path: &Path, text: &str) -> Result<Output, String> {
        let parent = path.parent().unwrap_or(Path::new("."));
        let out_dir = out_dir_for(path);
        fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        remove_stale_mirrors(parent);

        let mirror = Mirror::write(path, text)?;
        self.0.compile(&mirror.path, &out_dir, parent)?;
        fs::read(out_dir.join(format!("{}.pdf", mirror.stem)))
            .map(Output::Pdf)
            .map_err(|e| format!("cannot read the PDF: {e}"))
    }
}

fn mirror_name(source: &Path) -> Option<String> {
    Some(format!("{MIRROR_PREFIX}{}.tex", source.file_stem()?.to_str()?))
}

struct Mirror {
    path: PathBuf,
    stem: String,
}

impl Mirror {
    fn write(source: &Path, text: &str) -> Result<Self, String> {
        let name = mirror_name(source).ok_or("invalid file name")?;
        let stem = name.trim_end_matches(".tex").to_owned();
        let beside = source.parent().unwrap_or(Path::new(".")).join(&name);
        if fs::write(&beside, text).is_ok() {
            return Ok(Self { path: beside, stem });
        }
        let dir = out_dir_for(source).join("buffer");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(&name);
        fs::write(&path, text).map_err(|e| format!("cannot write the unsaved text: {e}"))?;
        Ok(Self { path, stem })
    }
}

impl Drop for Mirror {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn remove_stale_mirrors(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(MIRROR_PREFIX) && name.ends_with(".tex")) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > STALE_MIRROR_AGE);
        if old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn read_synctex(path: &Path) -> Result<SyncTex, String> {
    let stem = path.file_stem().and_then(|s| s.to_str()).ok_or("invalid file name")?;
    let out_dir = out_dir_for(path);
    let mirror_stem = mirror_name(path).map(|n| n.trim_end_matches(".tex").to_owned()).unwrap_or_default();
    let newest = [stem.to_owned(), mirror_stem]
        .into_iter()
        .map(|s| out_dir.join(format!("{s}.synctex.gz")))
        .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, p)| p)
        .unwrap_or_else(|| out_dir.join(format!("{stem}.synctex.gz")));
    SyncTex::read(&newest).map_err(|_| "no SyncTeX data; reload and try again".to_string())
}

fn same_file_from(base: &Path, target: &Path, input: &str) -> bool {
    fn clean(path: &Path) -> PathBuf {
        path.components().filter(|c| !matches!(c, std::path::Component::CurDir)).collect()
    }
    clean(&resolve_input(base, input)) == clean(target)
}

fn out_dir_for(path: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    std::env::temp_dir()
        .join(format!("washi-{}", std::process::id()))
        .join(format!("{:016x}", hasher.finish()))
}

fn resolve_input(source: &Path, input: &str) -> PathBuf {
    let input = Path::new(input);
    if input.file_name().and_then(|n| n.to_str()) == mirror_name(source).as_deref() {
        return source.to_path_buf();
    }
    if input.is_absolute() {
        return input.to_path_buf();
    }
    source.parent().unwrap_or(Path::new(".")).join(input)
}

pub struct SystemTexEngine;

impl TexEngine for SystemTexEngine {
    fn compile(&self, source: &Path, out_dir: &Path, cwd: &Path) -> Result<(), String> {
        let text = fs::read_to_string(source).unwrap_or_default();
        let (tool, mut command) = command_for(source, out_dir).ok_or(
            "neither latexmk nor tectonic was found; install one, for example with `brew install tectonic`",
        )?;
        let job = Job::start(source);
        let timeout = process::timeout_from_env();
        let output = process::run(command.current_dir(cwd), timeout, job.cancelled()).map_err(|e| match e {
            RunError::Spawn(e) => format!("cannot start {tool}: {e}"),
            RunError::TimedOut(limit) => format!(
                "{tool} was stopped after {} seconds; set {} to change the limit",
                limit.as_secs(),
                process::TIMEOUT_ENV,
            ),
            RunError::Cancelled => "stopped because a newer render replaced it".to_string(),
        })?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = format!("{tool} failed:\n{stderr}\n{}", tail(&stdout, 4000));
        let hint = failure_hint(tool, &text, &format!("{stderr}\n{stdout}"), find_tool("biber").is_some());
        Err(match hint {
            Some(hint) => format!("{hint}\n\n{details}"),
            None => details,
        })
    }
}

fn uses_biber(text: &str) -> bool {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('%'))
        .any(|line| BIBLATEX.captures(line).is_some_and(|c| !c.get(1).is_some_and(|o| BIBTEX_BACKEND.is_match(o.as_str()))))
}

fn failure_hint(tool: &str, text: &str, output: &str, biber_found: bool) -> Option<String> {
    let out = output.to_lowercase();
    if out.contains("shell-escape") || out.contains("shell escape") {
        return Some(
            "This document needs shell escape (for example for minted). Washi does not enable it, because it lets a document run commands."
                .into(),
        );
    }
    if out.contains("platex2e") {
        return Some(if tool == "tectonic" {
            "This document needs pLaTeX or upLaTeX, which tectonic cannot build. Install TeX Live with latexmk and add a .latexmkrc that selects platex."
        } else {
            "latexmk built this with pdfLaTeX, but the document needs pLaTeX. Add a .latexmkrc such as: $latex = 'platex'; $dvipdf = 'dvipdfmx %O -o %D %S'; $pdf_mode = 3;"
        }
        .into());
    }
    let missing_file = out.contains("biber") || out.contains("no such file or directory") || out.contains(".bbl") || out.contains(".bcf");
    if uses_biber(text) && !biber_found && missing_file {
        return Some(
            "This document uses biblatex, which needs biber, and biber was not found. Install TeX Live (or MacTeX) with latexmk; tectonic cannot run biber."
                .into(),
        );
    }
    if !text.contains("\\documentclass") && (out.contains("undefined control sequence") || out.contains("missing \\begin{document}")) {
        return Some(
            "This file has no \\documentclass, so it is probably a chapter. Add `% !TEX root = main.tex` on its first line to build the main file instead."
                .into(),
        );
    }
    None
}
fn latexmk_args(source: &Path, out_dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-e", "$pdf_mode = 1 if !$pdf_mode;", "-synctex=1", "-interaction=nonstopmode", "-halt-on-error", "-outdir"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.push(out_dir.into());
    args.push(source.into());
    args
}

fn command_for(source: &Path, out_dir: &Path) -> Option<(&'static str, Command)> {
    if let Some(latexmk) = find_tool("latexmk") {
        let mut command = Command::new(latexmk);
        command.args(latexmk_args(source, out_dir));
        return Some(("latexmk", command));
    }
    let tectonic: PathBuf = find_tool("tectonic")?;
    let mut command = Command::new(tectonic);
    command
        .args(["-X", "compile", "--synctex", "--outdir"])
        .arg(out_dir)
        .arg(source);
    Some(("tectonic", command))
}

fn tail(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeEngine(Result<(), String>);

    impl TexEngine for FakeEngine {
        fn compile(&self, source: &Path, out_dir: &Path, _cwd: &Path) -> Result<(), String> {
            self.0.clone()?;
            let stem = source.file_stem().unwrap().to_str().unwrap();
            fs::write(out_dir.join(format!("{stem}.pdf")), b"%PDF-fake").unwrap();
            Ok(())
        }
    }

    #[test]
    fn returns_the_pdf_the_engine_produced() {
        let renderer = TexRenderer(FakeEngine(Ok(())));
        match renderer.render(Path::new("/tmp/washi-fake-doc.tex")).unwrap() {
            Output::Pdf(bytes) => assert_eq!(bytes, b"%PDF-fake"),
            Output::Html(_) => panic!("PDF を期待"),
        }
    }

    #[test]
    fn propagates_engine_errors() {
        let renderer = TexRenderer(FakeEngine(Err("boom".into())));
        let err = renderer.render(Path::new("/tmp/x.tex")).err().unwrap();
        assert_eq!(err, "boom");
    }

    #[test]
    fn output_directories_differ_per_source_file() {
        let a = out_dir_for(Path::new("/x/a.tex"));
        assert_ne!(a, out_dir_for(Path::new("/y/a.tex")));
        assert_eq!(a, out_dir_for(Path::new("/x/a.tex")));
    }

    #[test]
    fn relative_inputs_resolve_against_the_source_directory() {
        assert_eq!(resolve_input(Path::new("/d/main.tex"), "chap/one.tex"), PathBuf::from("/d/chap/one.tex"));
        assert_eq!(resolve_input(Path::new("/d/main.tex"), "/abs/x.tex"), PathBuf::from("/abs/x.tex"));
    }

    #[test]
    fn locate_reports_missing_synctex_data() {
        let renderer = TexRenderer(FakeEngine(Ok(())));
        assert!(renderer.locate(Path::new("/nowhere/never.tex"), 1, 10.0, 10.0).is_err());
    }

    #[test]
    fn tail_respects_char_boundaries() {
        assert_eq!(tail("あいうえお", 7), "えお");
        assert_eq!(tail("abc", 10), "abc");
    }

    #[test]
    #[cfg(unix)]
    #[ignore = "PATH と WASHI_COMPILE_TIMEOUT を書き換える"]
    fn hung_engine_is_stopped_by_the_timeout() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("washi-fake-latexmk-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("latexmk");
        fs::write(&fake, "#!/bin/sh\nmarker=\"$(dirname \"$0\")/ran\"\n[ -e \"$marker\" ] && exit 0\ntouch \"$marker\"\nsleep 60 & wait\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs = vec![dir.clone()];
        dirs.extend(std::env::split_paths(&path));
        std::env::set_var("PATH", std::env::join_paths(dirs).unwrap());
        std::env::set_var(process::TIMEOUT_ENV, "1");

        let started = std::time::Instant::now();
        let err = SystemTexEngine.compile(Path::new("/tmp/washi-hung.tex"), &dir, Path::new("/tmp")).unwrap_err();
        assert!(err.contains("after 1 seconds"), "{err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));

        fs::remove_file(dir.join("ran")).unwrap();
        std::env::set_var(process::TIMEOUT_ENV, "30");
        let source = PathBuf::from("/tmp/washi-hung-cancel.tex");
        let first = {
            let (dir, source) = (dir.clone(), source.clone());
            std::thread::spawn(move || SystemTexEngine.compile(&source, &dir, Path::new("/tmp")))
        };
        std::thread::sleep(std::time::Duration::from_millis(500));
        let started = std::time::Instant::now();
        SystemTexEngine.compile(&source, &dir, Path::new("/tmp")).unwrap();
        let err = first.join().unwrap().unwrap_err();
        assert!(err.contains("newer render"), "{err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        fs::remove_dir_all(&dir).ok();
    }

    use std::sync::Mutex;

    #[derive(Default)]
    struct SpyEngine {
        calls: Mutex<Vec<(PathBuf, PathBuf, String)>>,
    }

    impl TexEngine for SpyEngine {
        fn compile(&self, source: &Path, out_dir: &Path, cwd: &Path) -> Result<(), String> {
            let body = fs::read_to_string(source).unwrap_or_default();
            self.calls.lock().unwrap().push((source.to_path_buf(), cwd.to_path_buf(), body));
            let stem = source.file_stem().unwrap().to_str().unwrap();
            fs::write(out_dir.join(format!("{stem}.pdf")), b"%PDF-spy").unwrap();
            Ok(())
        }
    }

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-tex-buf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_buffer_is_compiled_from_a_hidden_sibling_in_the_documents_folder() {
        let dir = project("sibling");
        let path = dir.join("paper.tex");
        let renderer = TexRenderer(SpyEngine::default());
        let rendered = renderer.render_buffer(&path, "\\input{chapters/one}");
        match rendered.output.unwrap() {
            Output::Pdf(bytes) => assert_eq!(bytes, b"%PDF-spy"),
            Output::Html(_) => panic!("PDF を期待"),
        }
        let calls = renderer.0.calls.lock().unwrap();
        let (source, cwd, body) = &calls[0];
        assert_eq!(source, &dir.join(".washi-buf-paper.tex"));
        assert_eq!(cwd, &dir, "作業フォルダは元のフォルダ");
        assert_eq!(body, "\\input{chapters/one}", "コンパイル時のファイルの中身は、保存前の本文");
        assert!(!dir.join(".washi-buf-paper.tex").exists(), "終わったら消す");
        assert!(!path.exists(), "本物のファイルには何も書かない");
    }

    #[cfg(unix)]
    #[test]
    fn an_unwritable_folder_falls_back_to_a_temp_file_but_keeps_the_working_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = project("readonly");
        let path = dir.join("paper.tex");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let renderer = TexRenderer(SpyEngine::default());
        let rendered = renderer.render_buffer(&path, "body");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(rendered.output.is_ok(), "{:?}", rendered.output.err());
        let calls = renderer.0.calls.lock().unwrap();
        let (source, cwd, body) = &calls[0];
        assert_eq!(body, "body");
        assert_eq!(cwd, &dir);
        if source.starts_with(&dir) {
            return;
        }
        assert!(source.starts_with(out_dir_for(&path)), "{source:?}");
        assert!(!source.exists(), "終わったら消す");
    }

    #[test]
    fn the_hidden_sibling_is_mapped_back_to_the_real_file() {
        let real = Path::new("/proj/paper.tex");
        assert_eq!(resolve_input(real, "/proj/.washi-buf-paper.tex"), real);
        assert_eq!(resolve_input(real, ".washi-buf-paper.tex"), real);
        assert_eq!(resolve_input(real, "chapters/one.tex"), PathBuf::from("/proj/chapters/one.tex"));
        assert!(same_file_from(real, real, "/proj/.washi-buf-paper.tex"));
        assert!(same_file_from(real, real, "./paper.tex"));
        assert!(!same_file_from(real, real, "chapters/paper.tex"));
    }

    #[test]
    fn only_old_hidden_siblings_are_swept() {
        let dir = project("sweep");
        let old = dir.join(".washi-buf-old.tex");
        let fresh = dir.join(".washi-buf-fresh.tex");
        let unrelated = dir.join(".washi-keep.tex");
        for f in [&old, &fresh, &unrelated] {
            fs::write(f, "x").unwrap();
        }
        let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
        fs::File::options().write(true).open(&old).unwrap().set_modified(two_hours_ago).unwrap();
        fs::File::options().write(true).open(&unrelated).unwrap().set_modified(two_hours_ago).unwrap();
        remove_stale_mirrors(&dir);
        assert!(!old.exists() && fresh.exists() && unrelated.exists());
    }

    #[test]
    fn forward_search_reads_the_synctex_of_the_latest_buffer_compile() {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;
        let dir = project("forward");
        let path = dir.join("paper.tex");
        let out_dir = out_dir_for(&path);
        fs::create_dir_all(&out_dir).unwrap();
        let text = format!(
            "SyncTeX Version:1\nInput:1:{}\nOutput:pdf\nMagnification:1000\nUnit:1\nX Offset:0\nY Offset:0\nContent:\n!100\n{{1\n[1,68:4736287,52685372:29760291,47949085,0\n(1,10:4736287,8000000:29760291,800000,200000\ng1,10:9000000,8000000\n)\n]\n}}1\n",
            dir.join(".washi-buf-paper.tex").display()
        );
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(text.as_bytes()).unwrap();
        fs::write(out_dir.join(".washi-buf-paper.synctex.gz"), gz.finish().unwrap()).unwrap();

        let renderer = TexRenderer(SpyEngine::default());
        let position = renderer.locate_forward(&path, 10, 1).unwrap().expect("位置が出る");
        assert_eq!(position.page, 1);
        let back = renderer.locate(&path, 1, position.x + 1.0, position.y + 1.0).unwrap().expect("後方検索");
        assert_eq!((back.file, back.line), (path, 10), "隠しファイルは元のファイルとして返る");
    }

    #[test]
    fn buffer_dependency_dirs_follow_the_unsaved_text() {
        let dir = project("deps");
        fs::create_dir_all(dir.join("chapters")).unwrap();
        let renderer = TexRenderer(SpyEngine::default());
        assert_eq!(renderer.buffer_dependency_dirs(&dir.join("paper.tex"), "\\input{chapters/one}"), vec![dir.join("chapters")]);
    }

    fn multi_file_project(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = project(name);
        fs::create_dir_all(dir.join("chapters")).unwrap();
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
    fn a_chapter_with_a_root_comment_is_built_through_the_main_file() {
        let (dir, main, chapter) = multi_file_project("root-build");
        let renderer = TexRenderer(SpyEngine::default());
        match renderer.render(&chapter).unwrap() {
            Output::Pdf(bytes) => assert_eq!(bytes, b"%PDF-spy"),
            Output::Html(_) => panic!("expected a PDF"),
        }
        let calls = renderer.0.calls.lock().unwrap();
        assert_eq!(calls[0].0, main, "the main file is compiled");
        assert_eq!(calls[0].1, dir, "in the main file's folder");
        drop(calls);
        assert_eq!(renderer.dependency_dirs(&chapter), renderer.dependency_dirs(&main), "the main file's includes are watched");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn an_unsaved_chapter_shows_the_last_build_instead_of_rebuilding_the_project() {
        let (dir, main, chapter) = multi_file_project("root-buffer");
        let disk = fs::read_to_string(&chapter).unwrap();
        let renderer = TexRenderer(SpyEngine::default());
        assert!(renderer.render_buffer(&chapter, &format!("{disk}\nedited")).output.is_ok(), "builds once when there is nothing to show");
        assert_eq!(renderer.0.calls.lock().unwrap().len(), 1);
        assert!(renderer.render_buffer(&chapter, &format!("{disk}\nedited more")).output.is_ok());
        assert_eq!(renderer.0.calls.lock().unwrap().len(), 1, "unsaved text reuses the last build");
        assert!(renderer.render_buffer(&chapter, &disk).output.is_ok());
        let calls = renderer.0.calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "text equal to the disk (just saved) rebuilds");
        assert!(calls.iter().all(|c| c.0 == main));
        drop(calls);
        assert_eq!(renderer.buffer_dependency_dirs(&chapter, &disk), renderer.dependency_dirs(&main));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn synctex_inputs_of_a_chapter_resolve_against_the_main_files_folder() {
        let root = Path::new("/proj/main.tex");
        let chapter = Path::new("/proj/chapters/one.tex");
        assert!(same_file_from(root, chapter, "chapters/one.tex"));
        assert!(same_file_from(root, chapter, "./chapters/one.tex"));
        assert!(!same_file_from(root, chapter, "one.tex"));
    }

    #[test]
    fn latexmk_leaves_the_engine_to_the_rc_file() {
        let args: Vec<String> = latexmk_args(Path::new("/d/a.tex"), Path::new("/o")).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!args.iter().any(|a| a == "-pdf"), "-pdf would override $pdf_mode from .latexmkrc: {args:?}");
        let at = args.iter().position(|a| a == "-e").expect("an -e argument");
        assert_eq!(args[at + 1], "$pdf_mode = 1 if !$pdf_mode;");
        assert_eq!(args.last().unwrap(), "/d/a.tex");
        assert!(args.windows(2).any(|w| w[0] == "-outdir" && w[1] == "/o"));
    }

    #[test]
    fn biblatex_is_recognised_unless_it_uses_the_bibtex_backend() {
        assert!(uses_biber("\\usepackage{biblatex}"));
        assert!(uses_biber("\\usepackage[style=numeric,backend=biber]{biblatex}"));
        assert!(uses_biber("\\RequirePackage[sorting=ynt]{biblatex}"));
        assert!(!uses_biber("\\usepackage[backend=bibtex]{biblatex}"));
        assert!(!uses_biber("% \\usepackage{biblatex}\n\\bibliography{refs}"));
        assert!(!uses_biber("\\usepackage{natbib}"));
    }

    #[test]
    fn failures_come_with_a_plain_first_line() {
        let biblatex = "\\documentclass{article}\\usepackage{biblatex}";
        let hint = failure_hint("tectonic", biblatex, "error: No such file or directory (os error 2)", false).unwrap();
        assert!(hint.contains("biber was not found"), "{hint}");
        assert!(failure_hint("tectonic", biblatex, "error: No such file or directory (os error 2)", true).is_none(), "biber is installed: no biber hint");
        assert!(failure_hint("tectonic", "\\documentclass{article}", "error: No such file or directory", false).is_none(), "not a biblatex document");

        let minted = failure_hint("tectonic", "", "Package minted Error: You must invoke LaTeX with the -shell-escape flag.", true).unwrap();
        assert!(minted.contains("shell escape"), "{minted}");

        let platex = "LaTeX Error: This file needs format `pLaTeX2e' but this is `pdfLaTeX'.";
        assert!(failure_hint("tectonic", "", platex, true).unwrap().contains("tectonic cannot build"));
        assert!(failure_hint("latexmk", "", platex, true).unwrap().contains(".latexmkrc"));

        let chapter = failure_hint("tectonic", "\\section{One}", "! Undefined control sequence.", true).unwrap();
        assert!(chapter.contains("% !TEX root"), "{chapter}");
        assert!(failure_hint("tectonic", "\\documentclass{article}", "! Undefined control sequence.", true).is_none(), "a real document with a typo gets no hint");
    }

    #[test]
    #[ignore = "runs a real tectonic and needs the network on first use"]
    fn real_tectonic_builds_a_chapter_through_its_root() {
        let (dir, _main, chapter) = multi_file_project("real-root");
        let renderer = TexRenderer(SystemTexEngine);
        match renderer.render(&chapter).unwrap() {
            Output::Pdf(bytes) => assert!(bytes.starts_with(b"%PDF"), "not a PDF"),
            Output::Html(_) => panic!("expected a PDF"),
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    #[ignore = "runs a real tectonic and needs the network on first use"]
    fn real_tectonic_explains_why_biblatex_failed() {
        if find_tool("biber").is_some() {
            return;
        }
        let dir = project("real-biber");
        let doc = dir.join("paper.tex");
        fs::write(&doc, "\\documentclass{article}\\usepackage[backend=biber]{biblatex}\\addbibresource{r.bib}\\begin{document}\\cite{k}\\printbibliography\\end{document}").unwrap();
        fs::write(dir.join("r.bib"), "@book{k,author={A},title={T},year={2000},publisher={P}}").unwrap();
        let err = TexRenderer(SystemTexEngine).render(&doc).err().expect("biblatex fails without biber");
        assert!(err.lines().next().unwrap().contains("biber was not found"), "{err}");
        fs::remove_dir_all(dir).ok();
    }
}
