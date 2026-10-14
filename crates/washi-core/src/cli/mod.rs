mod parse;
mod report;
#[cfg(test)]
mod tests;

use std::{
    fs,
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command as Process, Stdio},
};

pub use parse::{parse, wants_json, Command, Target};
pub use report::{
    exit_code, CliError, ErrorKind, ErrorReport, FileEntry, FormatEntry, FormatsReport, OpenReport, Tools, EXIT_INVALID,
    EXIT_LAUNCH_FAILED, EXIT_OK, FORMAT_VERSION,
};

use crate::{
    editor,
    render::{self, tools::find_tool, SourceLocation},
};

const HELP: &str = "washi - open Markdown, Typst, LaTeX and Mermaid in one window

USAGE
  washi <file>...              Open files in Washi and return immediately
  washi open <file>...         Same, for files named like a subcommand
  washi - [--name NAME]        Read stdin and open it as a document
                               (re-piping with the same --name updates the open window)
  washi formats [--json]       List supported formats and available tools
  washi install [--dir DIR]    Link this binary as `washi` into DIR (default ~/.local/bin)
  washi --gui [file]...        Run the app in the foreground
  washi                        Same as --gui (this is how Finder starts the app)

OPTIONS
  --json        Machine-readable output: one JSON object per run, on stdout when it
                succeeded and on stderr when it failed (see the README)
  --no-launch   Validate and report, but do not open anything
  -h, --help    Show this help
  -V, --version Show the version

EXIT CODES
  0  success
  1  the app could not be launched
  2  invalid usage, or a file is missing / unsupported
  Only 0 means success; other codes may be added in the future.

ENVIRONMENT
  WASHI_EDITOR           Editor command for cmd-click source jumps,
                         e.g. \"code -g {file}:{line}:{column}\"
  WASHI_COMPILE_TIMEOUT  Seconds before a LaTeX conversion is stopped (default 300)
";

pub trait Launcher {
    fn launch(&self, paths: &[PathBuf]) -> Result<(), String>;
}

pub struct SystemLauncher;

impl Launcher for SystemLauncher {
    fn launch(&self, paths: &[PathBuf]) -> Result<(), String> {
        let exe = fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        match app_bundle(&exe).filter(|_| cfg!(target_os = "macos")) {
            Some(bundle) => {
                let status = Process::new("open")
                    .arg("-a")
                    .arg(bundle)
                    .args(paths)
                    .status()
                    .map_err(|e| format!("could not run `open`: {e}"))?;
                status.success().then_some(()).ok_or_else(|| format!("`open` failed: {status}"))
            }
            None => {
                let mut command = Process::new(exe);
                command
                    .arg("--gui")
                    .args(paths)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                detach(&mut command);
                command.spawn().map(|_| ()).map_err(|e| format!("could not start Washi: {e}"))
            }
        }
    }
}

#[cfg(unix)]
fn detach(command: &mut Process) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn detach(_: &mut Process) {}

#[cfg(unix)]
fn link_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn link_file(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "not supported on this platform"))
}

pub fn app_bundle(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

pub struct Env<'a> {
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub temp: PathBuf,
    pub exe: PathBuf,
    pub path_var: Option<String>,
    pub stdin: &'a mut dyn Read,
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub launcher: &'a dyn Launcher,
    pub which: &'a dyn Fn(&str) -> Option<PathBuf>,
}

pub fn run(args: Vec<String>) -> Option<i32> {
    let command = match parse(&args) {
        Ok(Command::Gui) => return None,
        Ok(command) => command,
        Err(error) => {
            let mut stderr = std::io::stderr();
            return Some(fail(&mut stderr, wants_json(&args), vec![error]));
        }
    };
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let mut env = Env {
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        home: std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default(),
        temp: std::env::temp_dir(),
        exe: std::env::current_exe().unwrap_or_default(),
        path_var: std::env::var("PATH").ok(),
        stdin: &mut stdin,
        out: &mut stdout,
        err: &mut stderr,
        launcher: &SystemLauncher,
        which: &find_tool,
    };
    Some(execute(command, &mut env))
}

pub fn execute(command: Command, env: &mut Env) -> i32 {
    match command {
        Command::Gui => EXIT_OK,
        Command::Help => print(env, HELP.trim_end()),
        Command::Version => print(env, &format!("washi {}", env!("CARGO_PKG_VERSION"))),
        Command::Formats { json } => formats(env, json),
        Command::Install { dir } => install(env, dir),
        Command::Open { targets, json, launch } => open(env, targets, json, launch),
    }
}

fn print(env: &mut Env, text: &str) -> i32 {
    let _ = writeln!(env.out, "{text}");
    EXIT_OK
}

fn print_json(env: &mut Env, report: &impl serde::Serialize) -> i32 {
    let text = serde_json::to_string(report).expect("a report always serializes");
    print(env, &text)
}

fn fail(err: &mut dyn Write, json: bool, errors: Vec<CliError>) -> i32 {
    if json {
        let report = ErrorReport { format_version: FORMAT_VERSION, errors: &errors };
        let text = serde_json::to_string(&report).expect("a report always serializes");
        let _ = writeln!(err, "{text}");
    } else {
        for error in &errors {
            let _ = writeln!(err, "washi: {}", error.message);
        }
    }
    exit_code(&errors)
}

fn formats(env: &mut Env, json: bool) -> i32 {
    let tool = |name: &str| (env.which)(name).map(|p| p.to_string_lossy().into_owned());
    let location = SourceLocation { file: PathBuf::from("file"), line: 1, column: 1 };
    let editor = editor::plan(std::env::var(editor::EDITOR_ENV).ok().as_deref(), env.which, &location).program;
    let pandoc = tool("pandoc");
    let tex = ["pdflatex", "lualatex", "xelatex", "tectonic"].iter().find_map(|name| tool(name));

    let formats = render::formats();
    let mut extensions: Vec<&'static str> = formats.iter().flat_map(|(_, exts)| exts.iter().copied()).collect();
    extensions.sort_unstable();

    if json {
        let report = FormatsReport {
            format_version: FORMAT_VERSION,
            version: env!("CARGO_PKG_VERSION"),
            formats: formats
                .into_iter()
                .map(|(name, exts)| FormatEntry { name, extensions: exts.to_vec() })
                .collect(),
            extensions,
            typst: "built-in",
            tools: Tools { pandoc: pandoc.clone(), tex: tex.clone() },
            source_jump_editor: editor,
        };
        return print_json(env, &report);
    }
    let lines = [
        format!("extensions: {}", extensions.join(" ")),
        "typst: built-in".to_owned(),
        format!("latex converter: {}", pandoc.as_deref().unwrap_or("none (install pandoc)")),
        format!("tikz drawings: {}", tex.as_deref().unwrap_or("none (install tectonic)")),
        format!("source-jump editor: {editor}"),
    ];
    print(env, &lines.join("\n"))
}

fn install(env: &mut Env, dir: Option<PathBuf>) -> i32 {
    let dir = dir.unwrap_or_else(|| env.home.join(".local/bin"));
    let target = fs::canonicalize(&env.exe).unwrap_or_else(|_| env.exe.clone());
    let link = dir.join("washi");

    let result = (|| -> Result<(), String> {
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        match fs::symlink_metadata(&link) {
            Ok(meta) if meta.file_type().is_symlink() => fs::remove_file(&link).map_err(|e| e.to_string())?,
            Ok(_) => return Err(format!("{} exists and is not a symlink; remove it first", link.display())),
            Err(_) => {}
        }
        link_file(&target, &link).map_err(|e| format!("{}: {e}", link.display()))
    })();

    match result {
        Ok(()) => {
            let _ = writeln!(env.out, "installed: {} -> {}", link.display(), target.display());
            let on_path = env
                .path_var
                .as_deref()
                .is_some_and(|p| std::env::split_paths(p).any(|d| d == dir));
            if !on_path {
                let _ = writeln!(env.out, "note: {} is not on your PATH; add it to use `washi` everywhere", dir.display());
            }
            EXIT_OK
        }
        Err(message) => fail(env.err, false, vec![CliError::new(ErrorKind::Io, message)]),
    }
}

fn open(env: &mut Env, targets: Vec<Target>, json: bool, launch: bool) -> i32 {
    let mut paths = Vec::new();
    let mut errors = Vec::new();
    for target in targets {
        match resolve(env, target) {
            Ok(path) => paths.push(path),
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        return fail(env.err, json, errors);
    }
    if launch {
        if let Err(message) = env.launcher.launch(&paths) {
            return fail(env.err, json, vec![CliError::new(ErrorKind::LaunchFailed, message)]);
        }
    }
    let shown: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    if json {
        let files = paths
            .iter()
            .zip(&shown)
            .map(|(path, shown)| FileEntry {
                path: shown.clone(),
                format: render::format_of(path).unwrap_or("unknown"),
            })
            .collect();
        print_json(env, &OpenReport { format_version: FORMAT_VERSION, launched: launch, files })
    } else {
        let verb = if launch { "opened" } else { "ok" };
        let text = shown.iter().map(|p| format!("{verb} {p}")).collect::<Vec<_>>().join("\n");
        print(env, &text)
    }
}

fn resolve(env: &mut Env, target: Target) -> Result<PathBuf, CliError> {
    match target {
        Target::Stdin { name } => stage_stdin(env, name.as_deref()),
        Target::Path(raw) => {
            let path = if Path::new(&raw).is_absolute() { PathBuf::from(&raw) } else { env.cwd.join(&raw) };
            let path = fs::canonicalize(&path)
                .map_err(|_| CliError::at(ErrorKind::NotFound, format!("not found: {raw}"), raw.clone()))?;
            if path.is_dir() {
                return Err(CliError::at(ErrorKind::IsDirectory, format!("is a directory: {raw}"), raw));
            }
            if render::renderer_for(&path).is_none() {
                return Err(CliError::at(
                    ErrorKind::UnsupportedFormat,
                    format!("unsupported format: {raw} (supported: {})", render::supported_extensions().join(", ")),
                    raw,
                ));
            }
            Ok(path)
        }
    }
}

fn stage_stdin(env: &mut Env, name: Option<&str>) -> Result<PathBuf, CliError> {
    let io = |e: std::io::Error| CliError::new(ErrorKind::Io, e.to_string());
    let mut text = String::new();
    env.stdin
        .read_to_string(&mut text)
        .map_err(|e| CliError::new(ErrorKind::StdinUnreadable, format!("could not read stdin: {e}")))?;
    if text.trim().is_empty() {
        return Err(CliError::new(ErrorKind::StdinEmpty, "stdin is empty"));
    }
    let stem = match name.map(sanitize).filter(|n| !n.is_empty()) {
        Some(name) => name,
        None => {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            format!("stdin-{:08x}", hasher.finish() as u32)
        }
    };
    let dir = env.temp.join("washi-stdin");
    fs::create_dir_all(&dir).map_err(io)?;
    let file = dir.join(format!("{stem}.{}", render::extension_of_text(&text)));
    fs::write(&file, text).map_err(io)?;
    fs::canonicalize(&file).map_err(io)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}
