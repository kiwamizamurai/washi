use super::*;
use serde_json::Value;
use std::{cell::RefCell, io::Cursor};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[derive(Default)]
struct FakeLauncher {
    calls: RefCell<Vec<Vec<PathBuf>>>,
    fail: bool,
}

impl Launcher for FakeLauncher {
    fn launch(&self, paths: &[PathBuf]) -> Result<(), String> {
        self.calls.borrow_mut().push(paths.to_vec());
        if self.fail { Err("boom".into()) } else { Ok(()) }
    }
}

struct Harness {
    dir: PathBuf,
    launcher: FakeLauncher,
}

impl Harness {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("washi-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { dir, launcher: FakeLauncher::default() }
    }

    fn run(&self, list: &[&str], stdin: &str) -> (i32, String, String) {
        let argv = args(list);
        let (mut input, mut out, mut err) = (Cursor::new(stdin.as_bytes().to_vec()), Vec::new(), Vec::new());
        let which = |_: &str| None;
        let code = match parse(&argv) {
            Err(error) => fail(&mut err, wants_json(&argv), vec![error]),
            Ok(command) => {
                let mut env = Env {
                    cwd: self.dir.clone(),
                    home: self.dir.join("home"),
                    temp: self.dir.join("tmp"),
                    exe: self.dir.join("bin/washi"),
                    path_var: Some("/usr/bin".into()),
                    stdin: &mut input,
                    out: &mut out,
                    err: &mut err,
                    launcher: &self.launcher,
                    which: &which,
                };
                execute(command, &mut env)
            }
        };
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    fn file(&self, name: &str, body: &str) -> PathBuf {
        let path = self.dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        fs::canonicalize(path).unwrap()
    }
}

fn json(text: &str) -> Value {
    assert_eq!(text.trim_end().lines().count(), 1, "JSON は 1 行: {text:?}");
    serde_json::from_str(text).unwrap_or_else(|e| panic!("JSON ではない: {e}: {text:?}"))
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value.as_object().expect("オブジェクト").keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

#[test]
fn no_arguments_or_gui_flag_start_the_app() {
    assert_eq!(parse(&args(&[])).unwrap(), Command::Gui);
    assert_eq!(parse(&args(&["--gui", "a.md"])).unwrap(), Command::Gui);
}

#[test]
fn parses_subcommands_and_flags() {
    assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
    assert_eq!(parse(&args(&["-V"])).unwrap(), Command::Version);
    assert_eq!(parse(&args(&["formats", "--json"])).unwrap(), Command::Formats { json: true });
    assert_eq!(parse(&args(&["formats"])).unwrap(), Command::Formats { json: false });
    assert_eq!(
        parse(&args(&["install", "--dir", "/x"])).unwrap(),
        Command::Install { dir: Some(PathBuf::from("/x")) }
    );
    assert_eq!(
        parse(&args(&["a.md", "--json", "--no-launch", "b.typ"])).unwrap(),
        Command::Open {
            targets: vec![Target::Path("a.md".into()), Target::Path("b.typ".into())],
            json: true,
            launch: false
        }
    );
}

#[test]
fn open_subcommand_allows_files_named_like_subcommands() {
    assert_eq!(
        parse(&args(&["open", "formats"])).unwrap(),
        Command::Open { targets: vec![Target::Path("formats".into())], json: false, launch: true }
    );
    assert_eq!(
        parse(&args(&["--", "--weird.md"])).unwrap(),
        Command::Open { targets: vec![Target::Path("--weird.md".into())], json: false, launch: true }
    );
}

#[test]
fn rejects_bad_usage() {
    let message = |list: &[&str]| parse(&args(list)).unwrap_err().message;
    assert!(message(&["--bogus"]).contains("unknown option"));
    assert!(message(&["--json"]).contains("no files"));
    assert!(message(&["a.md", "--name", "x"]).contains("--name"));
    assert!(message(&["install", "--dir"]).contains("--dir"));
    assert!(message(&["formats", "--bogus"]).contains("unknown option for formats"));
    assert!(message(&["formats", "extra"]).contains("unknown option for formats"));
    assert!(message(&["-", "-"]).contains("only be given once"));
    assert!(parse(&args(&["--bogus"])).unwrap_err().kind == ErrorKind::Usage);
}

#[test]
fn json_is_detected_only_before_the_double_dash() {
    assert!(wants_json(&args(&["--bogus", "--json"])));
    assert!(!wants_json(&args(&["--", "--json"])));
    assert!(!wants_json(&args(&["a.md"])));
}

#[test]
fn opens_relative_files_by_absolute_path_and_launches_once() {
    let h = Harness::new("open");
    let a = h.file("docs/a.md", "# a");
    let b = h.file("b.typ", "= b");
    let (code, out, err) = h.run(&["docs/a.md", "b.typ"], "");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""));
    assert_eq!(h.launcher.calls.borrow().as_slice(), [vec![a.clone(), b.clone()]]);
    assert_eq!(out, format!("opened {}\nopened {}\n", a.display(), b.display()));
}

#[test]
fn invalid_files_are_reported_together_and_nothing_is_launched() {
    let h = Harness::new("invalid");
    h.file("ok.md", "x");
    h.file("notes.txt", "x");
    fs::create_dir_all(h.dir.join("folder")).unwrap();
    let (code, out, err) = h.run(&["ok.md", "missing.md", "notes.txt", "folder"], "");
    assert_eq!((code, out.as_str()), (EXIT_INVALID, ""));
    assert!(err.contains("not found: missing.md"), "{err}");
    assert!(err.contains("unsupported format: notes.txt"), "{err}");
    assert!(err.contains("is a directory: folder"), "{err}");
    assert!(h.launcher.calls.borrow().is_empty());
}

#[test]
fn no_launch_validates_without_opening() {
    let h = Harness::new("nolaunch");
    h.file("a.md", "x");
    let (code, out, _) = h.run(&["a.md", "--no-launch"], "");
    assert_eq!(code, EXIT_OK);
    assert!(out.starts_with("ok "), "{out}");
    assert!(h.launcher.calls.borrow().is_empty());
}

#[test]
fn launch_failure_has_its_own_exit_code() {
    let mut h = Harness::new("fail");
    h.launcher.fail = true;
    h.file("a.md", "x");
    let (code, out, err) = h.run(&["a.md"], "");
    assert_eq!((code, out.as_str()), (EXIT_LAUNCH_FAILED, ""));
    assert!(err.contains("boom"), "{err}");
}

#[test]
fn stdin_becomes_a_temp_document_with_a_detected_extension() {
    let h = Harness::new("stdin");
    let (code, out, _) = h.run(&["-"], "#set page(width: 10cm)\n= Hi\n");
    assert_eq!(code, EXIT_OK);
    let path = PathBuf::from(out.trim().strip_prefix("opened ").unwrap());
    assert_eq!(path.extension().unwrap(), "typ");
    assert!(path.starts_with(fs::canonicalize(h.dir.join("tmp")).unwrap()));
    assert_eq!(fs::read_to_string(path).unwrap(), "#set page(width: 10cm)\n= Hi\n");
}

#[test]
fn named_stdin_reuses_the_same_path_so_the_open_window_updates() {
    let h = Harness::new("named");
    let (_, first, _) = h.run(&["-", "--name", "plan v1"], "# one\n");
    let (_, second, _) = h.run(&["-", "--name", "plan v1"], "# two\n");
    assert_eq!(first, second);
    assert!(first.contains("plan-v1.md"), "{first}");
    let path = PathBuf::from(second.trim().strip_prefix("opened ").unwrap());
    assert_eq!(fs::read_to_string(path).unwrap(), "# two\n");
}

#[test]
fn empty_stdin_is_rejected() {
    let h = Harness::new("empty");
    let (code, _, err) = h.run(&["-"], "  \n");
    assert_eq!(code, EXIT_INVALID);
    assert!(err.contains("stdin is empty"), "{err}");
}

#[test]
fn help_and_version_print_to_stdout() {
    let h = Harness::new("help");
    let (code, out, err) = h.run(&["--help"], "");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""));
    assert!(out.contains("USAGE") && out.contains("EXIT CODES"));
    assert!(out.contains("Only 0 means success"), "終了コードは 0 だけを保証する");
    assert!(out.contains("same as --gui") || out.contains("Same as --gui"), "引数なしの挙動を書く");
    assert!(h.run(&["--version"], "").1.starts_with("washi "));
}

#[cfg(unix)]
#[test]
fn install_links_the_binary_and_replaces_an_old_link() {
    let h = Harness::new("install");
    let exe = h.file("bin/washi", "binary");
    let (code, out, _) = h.run(&["install", "--dir", h.dir.join("links").to_str().unwrap()], "");
    assert_eq!(code, EXIT_OK);
    assert!(out.contains("not on your PATH"), "{out}");
    let link = h.dir.join("links/washi");
    assert_eq!(fs::read_link(&link).unwrap(), exe);
    assert_eq!(h.run(&["install", "--dir", h.dir.join("links").to_str().unwrap()], "").0, EXIT_OK);
}

#[cfg(unix)]
#[test]
fn install_refuses_to_overwrite_a_regular_file() {
    let h = Harness::new("install-file");
    h.file("bin/washi", "binary");
    h.file("links/washi", "someone else's file");
    let (code, _, err) = h.run(&["install", "--dir", h.dir.join("links").to_str().unwrap()], "");
    assert_eq!(code, EXIT_INVALID);
    assert!(err.contains("not a symlink"), "{err}");
}

#[test]
fn finds_the_app_bundle_from_the_executable_path() {
    assert_eq!(
        app_bundle(Path::new("/Applications/Washi.app/Contents/MacOS/washi")),
        Some(PathBuf::from("/Applications/Washi.app"))
    );
    assert_eq!(app_bundle(Path::new("/x/target/debug/washi")), None);
}

#[test]
fn sanitize_keeps_names_filesystem_safe() {
    assert_eq!(sanitize("../etc/passwd"), "etc-passwd");
    assert_eq!(sanitize("計画 v2"), "計画-v2");
    assert_eq!(sanitize("///"), "");
}

#[test]
fn contract_open_success_json() {
    let h = Harness::new("c-open");
    let a = h.file("a.md", "# a");
    let t = h.file("b.typ", "= b");
    let (code, out, err) = h.run(&["a.md", "b.typ", "--json"], "");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""), "成功時の stderr は空");
    let v = json(&out);
    assert_eq!(keys(&v), ["files", "format_version", "launched"]);
    assert_eq!(v["format_version"], 1);
    assert_eq!(v["launched"], true);
    let files = v["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(keys(&files[0]), ["format", "path"]);
    assert_eq!(files[0]["path"], a.to_string_lossy().as_ref());
    assert_eq!(files[0]["format"], "markdown");
    assert_eq!(files[1]["path"], t.to_string_lossy().as_ref());
    assert_eq!(files[1]["format"], "typst");
}

#[test]
fn contract_no_launch_json_still_lists_the_validated_files() {
    let h = Harness::new("c-nolaunch");
    h.file("p.tex", "x");
    let (code, out, err) = h.run(&["p.tex", "--no-launch", "--json"], "");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""));
    let v = json(&out);
    assert_eq!(v["launched"], false);
    assert_eq!(v["files"][0]["format"], "latex");
    assert!(h.launcher.calls.borrow().is_empty());
}

#[test]
fn contract_every_format_name_appears_for_its_extension() {
    let h = Harness::new("c-formats-open");
    for (file, format) in [("a.md", "markdown"), ("a.mmd", "mermaid"), ("a.typ", "typst"), ("a.tex", "latex")] {
        h.file(file, "x");
        let (_, out, _) = h.run(&[file, "--no-launch", "--json"], "");
        assert_eq!(json(&out)["files"][0]["format"], format, "{file}");
    }
}

#[test]
fn contract_stdin_json_reports_the_detected_format() {
    let h = Harness::new("c-stdin");
    let (code, out, err) = h.run(&["-", "--name", "n", "--json"], "# hi\n");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""));
    let v = json(&out);
    assert_eq!(v["files"][0]["format"], "markdown");
    assert!(v["files"][0]["path"].as_str().unwrap().ends_with("n.md"));
}

#[test]
fn contract_failures_are_json_on_stderr_with_an_empty_stdout() {
    let h = Harness::new("c-errors");
    fs::create_dir_all(h.dir.join("folder")).unwrap();
    h.file("notes.txt", "x");
    let cases: [(&[&str], &str, &str, Option<&str>); 3] = [
        (&["missing.md", "--json"], "", "not_found", Some("missing.md")),
        (&["folder", "--json"], "", "is_directory", Some("folder")),
        (&["notes.txt", "--json"], "", "unsupported_format", Some("notes.txt")),
    ];
    for (argv, stdin, kind, path) in cases {
        let (code, out, err) = h.run(argv, stdin);
        assert_eq!((code, out.as_str()), (EXIT_INVALID, ""), "{argv:?}: 失敗時の stdout は空");
        let v = json(&err);
        assert_eq!(keys(&v), ["errors", "format_version"], "{argv:?}");
        assert_eq!(v["format_version"], 1);
        let e = &v["errors"][0];
        assert_eq!(e["kind"], kind, "{argv:?}");
        assert!(e["message"].as_str().is_some_and(|m| !m.is_empty()));
        assert_eq!(e["path"].as_str(), path, "{argv:?}");
        assert_eq!(keys(e), ["kind", "message", "path"]);
    }

    let (code, out, err) = h.run(&["-", "--json"], " \n");
    assert_eq!((code, out.as_str()), (EXIT_INVALID, ""));
    let e = &json(&err)["errors"][0];
    assert_eq!(e["kind"], "stdin_empty");
    assert_eq!(keys(e), ["kind", "message"], "path が無いときはキーごと出さない");
}

#[test]
fn contract_all_problems_are_listed_in_one_report() {
    let h = Harness::new("c-many");
    h.file("ok.md", "x");
    h.file("nope.txt", "x");
    let (_, _, err) = h.run(&["missing.md", "ok.md", "nope.txt", "--json"], "");
    let v = json(&err);
    let kinds: Vec<&str> = v["errors"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["not_found", "unsupported_format"]);
}

#[test]
fn contract_launch_failure_is_json_with_exit_code_1() {
    let mut h = Harness::new("c-launch");
    h.launcher.fail = true;
    h.file("a.md", "x");
    let (code, out, err) = h.run(&["a.md", "--json"], "");
    assert_eq!((code, out.as_str()), (EXIT_LAUNCH_FAILED, ""));
    let e = &json(&err)["errors"][0];
    assert_eq!(e["kind"], "launch_failed");
    assert_eq!(e["message"], "boom");
}

#[test]
fn contract_usage_errors_are_json_only_when_json_was_asked_for() {
    let h = Harness::new("c-usage");
    let (code, out, err) = h.run(&["--bogus", "--json"], "");
    assert_eq!((code, out.as_str()), (EXIT_INVALID, ""));
    assert_eq!(json(&err)["errors"][0]["kind"], "usage");

    let (code, out, err) = h.run(&["--bogus"], "");
    assert_eq!((code, out.as_str()), (EXIT_INVALID, ""));
    assert!(err.starts_with("washi: unknown option: --bogus"), "{err}");
    assert!(serde_json::from_str::<Value>(&err).is_err(), "--json が無ければ文のまま");

    let (code, _, err) = h.run(&["formats", "--bogus", "--json"], "");
    assert_eq!(code, EXIT_INVALID);
    assert_eq!(json(&err)["errors"][0]["kind"], "usage");
}

#[test]
fn contract_exit_code_depends_only_on_the_kind_of_failure() {
    let launch = CliError::new(ErrorKind::LaunchFailed, "x");
    let missing = CliError::new(ErrorKind::NotFound, "x");
    assert_eq!(exit_code(&[missing.clone()]), EXIT_INVALID);
    assert_eq!(exit_code(&[launch.clone()]), EXIT_LAUNCH_FAILED);
    assert_eq!(exit_code(&[missing, launch]), EXIT_LAUNCH_FAILED);
    assert_eq!((EXIT_OK, EXIT_LAUNCH_FAILED, EXIT_INVALID), (0, 1, 2));
}

#[test]
fn contract_error_kind_names_match_their_serialization() {
    for kind in ErrorKind::ALL {
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{}\"", kind.as_str()));
    }
    let mut names: Vec<_> = ErrorKind::ALL.iter().map(|k| k.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), ErrorKind::ALL.len(), "kind は重複しない");
}

#[test]
fn contract_formats_json() {
    let h = Harness::new("c-formats");
    let (code, out, err) = h.run(&["formats", "--json"], "");
    assert_eq!((code, err.as_str()), (EXIT_OK, ""));
    let v = json(&out);
    assert_eq!(
        keys(&v),
        ["extensions", "format_version", "formats", "source_jump_editor", "tools", "typst", "version"]
    );
    assert_eq!(v["format_version"], 1);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["typst"], "built-in");
    assert_eq!(keys(&v["tools"]), ["pandoc", "tex"]);
    assert!(v["tools"]["pandoc"].is_null() && v["tools"]["tex"].is_null(), "null when the tool is not installed");
    assert_eq!(v["source_jump_editor"], "open");

    let formats = v["formats"].as_array().unwrap();
    let names: Vec<&str> = formats.iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["markdown", "mermaid", "typst", "latex"]);
    assert_eq!(keys(&formats[0]), ["extensions", "name"]);

    let extensions: Vec<&str> = v["extensions"].as_array().unwrap().iter().map(|e| e.as_str().unwrap()).collect();
    let mut sorted = extensions.clone();
    sorted.sort_unstable();
    assert_eq!(extensions, sorted, "extensions は昇順");
    for wanted in ["md", "typ", "tex", "mmd"] {
        assert!(extensions.contains(&wanted), "{wanted}");
    }
    assert!(!extensions.contains(&"pdf"), "PDF is not a supported format");
}

#[test]
fn contract_formats_reports_the_pandoc_that_would_run() {
    let h = Harness::new("c-formats-tex");
    let which = |name: &str| match name {
        "pandoc" => Some(PathBuf::from("/opt/pandoc")),
        "tectonic" => Some(PathBuf::from("/opt/tectonic")),
        _ => None,
    };
    let (mut input, mut out, mut err) = (Cursor::new(Vec::new()), Vec::new(), Vec::new());
    let mut env = Env {
        cwd: h.dir.clone(),
        home: h.dir.clone(),
        temp: h.dir.clone(),
        exe: h.dir.join("washi"),
        path_var: None,
        stdin: &mut input,
        out: &mut out,
        err: &mut err,
        launcher: &h.launcher,
        which: &which,
    };
    assert_eq!(execute(Command::Formats { json: true }, &mut env), EXIT_OK);
    let v = json(&String::from_utf8(out).unwrap());
    assert_eq!(v["tools"]["pandoc"], "/opt/pandoc");
    assert_eq!(v["tools"]["tex"], "/opt/tectonic", "the engine that would draw TikZ");
}

#[test]
fn readme_documents_the_contract() {
    let readme = include_str!("../../../../README.md");
    assert!(readme.contains("format_version"), "README に format_version が無い");
    for kind in ErrorKind::ALL {
        assert!(readme.contains(&format!("`{}`", kind.as_str())), "README に kind `{}` が無い", kind.as_str());
    }
    for (name, _) in render::formats() {
        assert!(readme.contains(&format!("`{name}`")), "README に format `{name}` が無い");
    }
    for needle in ["Exit codes", "Streams", "stdout", "stderr", "Compatibility"] {
        assert!(readme.contains(needle), "README に {needle} が無い");
    }
}
