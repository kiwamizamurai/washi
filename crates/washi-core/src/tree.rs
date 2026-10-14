use std::{fs, io, path::Path};

use crate::render;

const SKIPPED_DIRS: [&str; 1] = ["node_modules"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_depth: usize,
    pub max_visited: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_depth: 8, max_visited: 5000 }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub files: Vec<String>,
    pub truncated: bool,
}

struct Walk {
    limits: Limits,
    visited: usize,
    scan: Scan,
}

pub fn scan(root: &Path, limits: Limits) -> io::Result<Scan> {
    let mut walk = Walk { limits, visited: 0, scan: Scan::default() };
    walk.dir(root, "", 0)?;
    walk.scan.files.sort();
    Ok(walk.scan)
}

impl Walk {
    fn dir(&mut self, dir: &Path, prefix: &str, depth: usize) -> io::Result<()> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if self.visited >= self.limits.max_visited {
                self.scan.truncated = true;
                return Ok(());
            }
            self.visited += 1;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() {
                if SKIPPED_DIRS.contains(&name) {
                    continue;
                }
                if depth >= self.limits.max_depth {
                    self.scan.truncated = true;
                    continue;
                }
                let _ = self.dir(&path, &format!("{prefix}{name}/"), depth + 1);
            } else if is_document(&path, kind.is_symlink()) {
                self.scan.files.push(format!("{prefix}{name}"));
            }
        }
        Ok(())
    }
}

pub fn contains(root: &Path, path: &Path) -> bool {
    match (fs::canonicalize(root), fs::canonicalize(path)) {
        (Ok(root), Ok(path)) => path.starts_with(root),
        _ => false,
    }
}

fn is_document(path: &Path, symlink: bool) -> bool {
    let is_file = if symlink {
        fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
    } else {
        true
    };
    is_file && render::renderer_for(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tree(name: &str, files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("washi-tree-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        root
    }

    fn files(root: &Path) -> Vec<String> {
        scan(root, Limits::default()).unwrap().files
    }

    #[test]
    fn lists_only_formats_washi_can_read() {
        let root = tree("formats", &["a.md", "b.typ", "c.tex", "d.pdf", "e.mmd", "notes.txt", "f.docx", "Makefile"]);
        assert_eq!(files(&root), ["a.md", "b.typ", "c.tex", "e.mmd"]);
    }

    #[test]
    fn matches_extensions_case_insensitively() {
        let root = tree("case", &["README.MD", "Paper.Tex"]);
        assert_eq!(files(&root), ["Paper.Tex", "README.MD"]);
    }

    #[test]
    fn descends_into_subfolders_with_relative_paths() {
        let root = tree("nested", &["top.md", "docs/guide.md", "docs/api/ref.typ"]);
        assert_eq!(files(&root), ["docs/api/ref.typ", "docs/guide.md", "top.md"]);
    }

    #[test]
    fn skips_hidden_entries_and_node_modules() {
        let root = tree(
            "skip",
            &[".hidden.md", ".git/readme.md", ".washi-buf-main.tex", "node_modules/pkg/readme.md", "kept.md"],
        );
        assert_eq!(files(&root), ["kept.md"]);
    }

    #[test]
    fn leaves_out_folders_without_documents() {
        let root = tree("empty", &["images/a.png", "src/main.rs", "doc.md"]);
        fs::create_dir_all(root.join("blank")).unwrap();
        assert_eq!(files(&root), ["doc.md"]);
    }

    #[test]
    fn is_sorted_whatever_order_the_disk_returns() {
        let root = tree("sorted", &["b.md", "a.md", "c/z.md", "c/y.md"]);
        assert_eq!(files(&root), ["a.md", "b.md", "c/y.md", "c/z.md"]);
    }

    #[test]
    fn stops_at_the_depth_limit_and_says_so() {
        let root = tree("depth", &["a/one.md", "a/b/two.md"]);
        let limits = Limits { max_depth: 1, ..Limits::default() };
        let scan = scan(&root, limits).unwrap();
        assert_eq!(scan.files, ["a/one.md"]);
        assert!(scan.truncated);
    }

    #[test]
    fn stops_at_the_entry_limit_and_says_so() {
        let names: Vec<String> = (0..20).map(|i| format!("{i:02}.md")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let root = tree("limit", &refs);
        let limits = Limits { max_visited: 5, ..Limits::default() };
        let scan = scan(&root, limits).unwrap();
        assert_eq!(scan.files, ["00.md", "01.md", "02.md", "03.md", "04.md"]);
        assert!(scan.truncated);
    }

    #[test]
    fn is_not_truncated_when_everything_fits() {
        let root = tree("fits", &["a.md", "b/c.md"]);
        assert!(!scan(&root, Limits::default()).unwrap().truncated);
    }

    #[test]
    fn fails_for_a_missing_folder() {
        let missing = std::env::temp_dir().join(format!("washi-tree-missing-{}", std::process::id()));
        assert!(scan(&missing, Limits::default()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_folder_symlinks() {
        let root = tree("loop", &["real.md", "sub/inner.md"]);
        std::os::unix::fs::symlink(&root, root.join("sub/back")).unwrap();
        std::os::unix::fs::symlink(root.join("sub"), root.join("alias")).unwrap();
        assert_eq!(files(&root), ["real.md", "sub/inner.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn includes_symlinked_documents_but_not_broken_links() {
        let root = tree("links", &["real.md"]);
        std::os::unix::fs::symlink(root.join("real.md"), root.join("link.md")).unwrap();
        std::os::unix::fs::symlink(root.join("gone.md"), root.join("broken.md")).unwrap();
        assert_eq!(files(&root), ["link.md", "real.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn skips_folders_it_cannot_read() {
        use std::os::unix::fs::PermissionsExt;
        let root = tree("denied", &["ok.md", "locked/secret.md"]);
        let locked = root.join("locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let enforced = fs::read_dir(&locked).is_err();
        let listed = files(&root);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if enforced {
            assert_eq!(listed, ["ok.md"]);
        }
    }

    #[test]
    fn contains_accepts_files_inside_the_root() {
        let root = tree("inside", &["a.md", "sub/b.md"]);
        assert!(contains(&root, &root.join("a.md")));
        assert!(contains(&root, &root.join("sub/b.md")));
    }

    #[test]
    fn contains_rejects_files_outside_or_missing() {
        let root = tree("outside", &["inner/a.md", "other.md"]);
        let inner = root.join("inner");
        assert!(!contains(&inner, &root.join("other.md")));
        assert!(!contains(&inner, &inner.join("../other.md")));
        assert!(!contains(&inner, &inner.join("missing.md")));
    }

    #[cfg(unix)]
    #[test]
    fn contains_rejects_symlinks_that_leave_the_root() {
        let root = tree("escape", &["inner/a.md", "secret.md"]);
        let inner = root.join("inner");
        std::os::unix::fs::symlink(root.join("secret.md"), inner.join("leak.md")).unwrap();
        assert!(!contains(&inner, &inner.join("leak.md")));
    }
}
