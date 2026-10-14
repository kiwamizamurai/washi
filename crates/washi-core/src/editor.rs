use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::render::{tools::find_tool, SourceLocation};

pub const EDITOR_ENV: &str = "WASHI_EDITOR";

const KNOWN_EDITORS: [(&str, &[&str]); 5] = [
    ("code", &["-g", "{file}:{line}:{column}"]),
    ("cursor", &["-g", "{file}:{line}:{column}"]),
    ("zed", &["{file}:{line}:{column}"]),
    ("subl", &["{file}:{line}:{column}"]),
    ("mate", &["-l", "{line}", "{file}"]),
];

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
}

fn fill(template: &str, file: &Path, line: usize, column: usize) -> String {
    template
        .replace("{file}", &file.to_string_lossy())
        .replace("{line}", &line.to_string())
        .replace("{column}", &column.to_string())
}

pub fn plan(
    custom: Option<&str>,
    find: &dyn Fn(&str) -> Option<PathBuf>,
    location: &SourceLocation,
) -> Invocation {
    let SourceLocation { file, line, column } = location;
    let render = |args: &[&str]| args.iter().map(|a| fill(a, file, *line, *column)).collect();

    if let Some(custom) = custom.filter(|c| !c.trim().is_empty()) {
        let mut parts = custom.split_whitespace();
        let program = parts.next().unwrap_or_default().to_owned();
        let args: Vec<&str> = parts.collect();
        return Invocation { program, args: render(&args) };
    }
    for (name, args) in KNOWN_EDITORS {
        if let Some(program) = find(name) {
            return Invocation { program: program.to_string_lossy().into_owned(), args: render(args) };
        }
    }
    Invocation { program: "open".into(), args: render(&["-t", "{file}"]) }
}

pub fn open(location: &SourceLocation) -> Result<(), String> {
    let custom = std::env::var(EDITOR_ENV).ok();
    let invocation = plan(custom.as_deref(), &find_tool, location);
    Command::new(&invocation.program)
        .args(&invocation.args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start the editor {}: {e}", invocation.program))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location() -> SourceLocation {
        SourceLocation { file: PathBuf::from("/my docs/a.typ"), line: 12, column: 5 }
    }

    fn only(names: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
        move |n| names.contains(&n).then(|| PathBuf::from(format!("/bin/{n}")))
    }

    #[test]
    fn prefers_vs_code_and_uses_line_and_column() {
        let plan = plan(None, &only(&["code", "cursor", "zed"]), &location());
        assert_eq!(plan.program, "/bin/code");
        assert_eq!(plan.args, ["-g", "/my docs/a.typ:12:5"]);
    }

    #[test]
    fn falls_through_the_known_editors_in_order() {
        assert_eq!(plan(None, &only(&["zed", "subl"]), &location()).program, "/bin/zed");
        let mate = plan(None, &only(&["mate"]), &location());
        assert_eq!(mate.args, ["-l", "12", "/my docs/a.typ"]);
    }

    #[test]
    fn falls_back_to_the_default_text_editor() {
        let plan = plan(None, &only(&[]), &location());
        assert_eq!(plan, Invocation { program: "open".into(), args: vec!["-t".into(), "/my docs/a.typ".into()] });
    }

    #[test]
    fn custom_template_wins_and_keeps_paths_with_spaces_intact() {
        let plan = plan(Some("nvim-remote --line {line} {file}"), &only(&["code"]), &location());
        assert_eq!(plan.program, "nvim-remote");
        assert_eq!(plan.args, ["--line", "12", "/my docs/a.typ"]);
    }

    #[test]
    fn blank_custom_template_is_ignored() {
        assert_eq!(plan(Some("  "), &only(&["code"]), &location()).program, "/bin/code");
    }
}
