use serde::Serialize;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Usage,
    NotFound,
    IsDirectory,
    UnsupportedFormat,
    StdinEmpty,
    StdinUnreadable,
    Io,
    LaunchFailed,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 8] = [
        Self::Usage,
        Self::NotFound,
        Self::IsDirectory,
        Self::UnsupportedFormat,
        Self::StdinEmpty,
        Self::StdinUnreadable,
        Self::Io,
        Self::LaunchFailed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::NotFound => "not_found",
            Self::IsDirectory => "is_directory",
            Self::UnsupportedFormat => "unsupported_format",
            Self::StdinEmpty => "stdin_empty",
            Self::StdinUnreadable => "stdin_unreadable",
            Self::Io => "io",
            Self::LaunchFailed => "launch_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliError {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl CliError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), path: None }
    }

    pub fn at(kind: ErrorKind, message: impl Into<String>, path: impl Into<String>) -> Self {
        Self { kind, message: message.into(), path: Some(path.into()) }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, message)
    }
}

pub const EXIT_OK: i32 = 0;
pub const EXIT_LAUNCH_FAILED: i32 = 1;
pub const EXIT_INVALID: i32 = 2;

pub fn exit_code(errors: &[CliError]) -> i32 {
    if errors.iter().any(|e| e.kind == ErrorKind::LaunchFailed) {
        EXIT_LAUNCH_FAILED
    } else {
        EXIT_INVALID
    }
}

#[derive(Serialize)]
pub struct ErrorReport<'a> {
    pub format_version: u32,
    pub errors: &'a [CliError],
}

#[derive(Serialize)]
pub struct FileEntry {
    pub path: String,
    pub format: &'static str,
}

#[derive(Serialize)]
pub struct OpenReport {
    pub format_version: u32,
    pub launched: bool,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize)]
pub struct FormatEntry {
    pub name: &'static str,
    pub extensions: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct Tools {
    pub pandoc: Option<String>,
    pub tex: Option<String>,
}

#[derive(Serialize)]
pub struct FormatsReport {
    pub format_version: u32,
    pub version: &'static str,
    pub formats: Vec<FormatEntry>,
    pub extensions: Vec<&'static str>,
    pub typst: &'static str,
    pub tools: Tools,
    pub source_jump_editor: String,
}
