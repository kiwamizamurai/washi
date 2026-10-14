use std::path::PathBuf;

use super::report::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Path(String),
    Stdin { name: Option<String> },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Gui,
    Help,
    Version,
    Formats { json: bool },
    Install { dir: Option<PathBuf> },
    Open { targets: Vec<Target>, json: bool, launch: bool },
}

pub fn wants_json(args: &[String]) -> bool {
    args.iter().take_while(|a| a.as_str() != "--").any(|a| a == "--json")
}

pub fn parse(args: &[String]) -> Result<Command, CliError> {
    let Some(first) = args.first().map(String::as_str) else {
        return Ok(Command::Gui);
    };
    match first {
        "--gui" => Ok(Command::Gui),
        "-h" | "--help" | "help" => Ok(Command::Help),
        "-V" | "--version" => Ok(Command::Version),
        "formats" => parse_formats(&args[1..]),
        "install" => parse_install(&args[1..]),
        "open" => parse_open(&args[1..]),
        _ => parse_open(args),
    }
}

fn parse_formats(args: &[String]) -> Result<Command, CliError> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other => return Err(CliError::usage(format!("unknown option for formats: {other}"))),
        }
    }
    Ok(Command::Formats { json })
}

fn parse_install(args: &[String]) -> Result<Command, CliError> {
    let mut dir = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dir" => {
                dir = Some(PathBuf::from(rest.next().ok_or_else(|| CliError::usage("--dir needs a value"))?))
            }
            other => return Err(CliError::usage(format!("unknown option for install: {other}"))),
        }
    }
    Ok(Command::Install { dir })
}

fn parse_open(args: &[String]) -> Result<Command, CliError> {
    let (mut json, mut launch, mut name) = (false, true, None);
    let mut targets = Vec::new();
    let mut flags_done = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--" if !flags_done => flags_done = true,
            "--json" if !flags_done => json = true,
            "--no-launch" if !flags_done => launch = false,
            "--name" if !flags_done => {
                name = Some(rest.next().ok_or_else(|| CliError::usage("--name needs a value"))?.clone())
            }
            "-" if !flags_done => targets.push(Target::Stdin { name: None }),
            flag if !flags_done && flag.starts_with('-') => {
                return Err(CliError::usage(format!("unknown option: {flag}")))
            }
            path => targets.push(Target::Path(path.to_owned())),
        }
    }
    if targets.iter().filter(|t| matches!(t, Target::Stdin { .. })).count() > 1 {
        return Err(CliError::usage("stdin (`-`) can only be given once"));
    }
    if let Some(name) = name {
        match targets.iter_mut().find(|t| matches!(t, Target::Stdin { .. })) {
            Some(Target::Stdin { name: slot }) => *slot = Some(name),
            _ => return Err(CliError::usage("--name only applies to `-` (stdin)")),
        }
    }
    if targets.is_empty() {
        return Err(CliError::usage("no files given (try `washi --help`)"));
    }
    Ok(Command::Open { targets, json, launch })
}
