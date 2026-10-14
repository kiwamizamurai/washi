use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

const FALLBACK_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/Library/TeX/texbin"];

fn fallback_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = FALLBACK_DIRS.iter().map(PathBuf::from).collect();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join(".cargo/bin"));
    }
    dirs
}

fn merge_path(current: Option<OsString>, extra: &[PathBuf]) -> OsString {
    let mut dirs: Vec<PathBuf> = current.map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    for dir in extra {
        if !dirs.contains(dir) {
            dirs.push(dir.clone());
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

pub fn path_with_fallbacks() -> OsString {
    merge_path(std::env::var_os("PATH"), &fallback_dirs())
}

pub fn find_tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&path_with_fallbacks()).map(|d| d.join(name)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fallback_directories_are_added_after_the_existing_path_without_duplicates() {
        let current = std::env::join_paths(["/usr/bin", "/opt/homebrew/bin"]).unwrap();
        let merged = merge_path(Some(current), &[PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/Library/TeX/texbin")]);
        let dirs: Vec<PathBuf> = std::env::split_paths(&merged).collect();
        assert_eq!(dirs, ["/usr/bin", "/opt/homebrew/bin", "/Library/TeX/texbin"].map(PathBuf::from));
    }

    #[test]
    fn a_missing_path_becomes_just_the_fallbacks() {
        let merged = merge_path(None, &[PathBuf::from("/opt/homebrew/bin")]);
        assert_eq!(std::env::split_paths(&merged).collect::<Vec<_>>(), [PathBuf::from("/opt/homebrew/bin")]);
    }

    #[test]
    fn the_fallbacks_include_where_homebrew_and_mactex_put_their_tools() {
        let dirs = fallback_dirs();
        assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
        assert!(dirs.contains(&PathBuf::from("/Library/TeX/texbin")));
    }
}
