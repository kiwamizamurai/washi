use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

use comrak::{nodes::NodeValue, parse_document, Arena};
use regex::Regex;

const MAX_FILES: usize = 64;

fn directories(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in paths {
        let dir = normalize(&dir);
        let too_broad = dir.parent().is_none() || home.as_deref() == Some(dir.as_path());
        if too_broad || !dir.is_dir() || !seen.insert(dir.clone()) {
            continue;
        }
        out.push(dir);
    }
    out
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_remote(target: &str) -> bool {
    target.contains("://") || target.starts_with("data:") || target.starts_with("mailto:")
}

pub fn markdown(source: &str, base: &Path) -> Vec<PathBuf> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &comrak::Options::default());
    let mut dirs = Vec::new();
    for node in root.descendants() {
        if let NodeValue::Image(link) = &node.data.borrow().value {
            let url = link.url.split(['#', '?']).next().unwrap_or("");
            if url.is_empty() || is_remote(url) {
                continue;
            }
            if let Some(parent) = base.join(url).parent() {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    directories(dirs)
}

static TEX_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\\(input|include|subfile|InputIfFileExists|bibliography|addbibresource|addglobalbib|addsectionbib|includegraphics|includesvg|includepdf|lstinputlisting|verbatiminput|usepackage|RequirePackage|documentclass)\*?\s*(?:\[[^\]]*\])?\s*\{([^}]*)\}",
    )
    .unwrap()
});
static TEX_INPUT_BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\input[ \t]+([^\s{}\\%]+)").unwrap());
static TEX_DIR_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\(?:import|subimport|includefrom|inputfrom|subincludefrom|subinputfrom)\*?\s*\{([^}]*)\}\s*\{([^}]*)\}").unwrap()
});
static TEX_INPUTMINTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\inputminted\*?\s*(?:\[[^\]]*\])?\s*\{[^}]*\}\s*\{([^}]*)\}").unwrap());
static TEX_GRAPHICSPATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\graphicspath\s*\{((?:\s*\{[^}]*\}\s*)+)\}").unwrap());
static BRACED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]*)\}").unwrap());

fn strip_tex_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let mut escaped = false;
            for (i, c) in line.char_indices() {
                match c {
                    '\\' => escaped = !escaped,
                    '%' if !escaped => return &line[..i],
                    _ => escaped = false,
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_source(file: &Path, main: &Path, main_text: Option<&str>) -> Option<String> {
    match main_text {
        Some(text) if file == main => Some(text.to_owned()),
        _ => fs::read(file).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
    }
}

fn rooted(root: &Path, target: &str) -> PathBuf {
    let target = Path::new(target.trim());
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    }
}

fn tex_file(root: &Path, target: &str) -> PathBuf {
    let path = rooted(root, target);
    if path.extension().is_some_and(|e| e == "tex") {
        path
    } else {
        let mut name = path.into_os_string();
        name.push(".tex");
        PathBuf::from(name)
    }
}

pub fn latex(main: &Path) -> Vec<PathBuf> {
    latex_text(main, None)
}

pub fn latex_text(main: &Path, main_text: Option<&str>) -> Vec<PathBuf> {
    let root = main.parent().unwrap_or(Path::new("."));
    let mut dirs = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = vec![main.to_path_buf()];

    while let Some(file) = queue.pop() {
        if visited.len() >= MAX_FILES || !visited.insert(file.clone()) {
            continue;
        }
        let Some(source) = read_source(&file, main, main_text) else { continue };
        let source = strip_tex_comments(&source);

        for caps in TEX_COMMAND.captures_iter(&source) {
            let command = &caps[1];
            for name in caps[2].split(',').map(|n| n.trim().trim_matches('"')).filter(|n| !n.is_empty()) {
                match command {
                    "input" | "include" | "subfile" | "InputIfFileExists" => {
                        let path = tex_file(root, name);
                        dirs.extend(path.parent().map(Path::to_path_buf));
                        queue.push(path);
                    }
                    "usepackage" | "RequirePackage" | "documentclass" => {
                        for ext in ["sty", "cls"] {
                            let path = rooted(root, &format!("{name}.{ext}"));
                            if path.is_file() {
                                dirs.extend(path.parent().map(Path::to_path_buf));
                            }
                        }
                    }
                    _ => dirs.extend(rooted(root, name).parent().map(Path::to_path_buf)),
                }
            }
        }
        for caps in TEX_INPUT_BARE.captures_iter(&source) {
            let path = tex_file(root, caps[1].trim_matches('"'));
            dirs.extend(path.parent().map(Path::to_path_buf));
            queue.push(path);
        }
        for caps in TEX_DIR_FILE.captures_iter(&source) {
            let path = tex_file(&rooted(root, caps[1].trim().trim_matches('"')), caps[2].trim().trim_matches('"'));
            dirs.extend(path.parent().map(Path::to_path_buf));
            queue.push(path);
        }
        for caps in TEX_INPUTMINTED.captures_iter(&source) {
            dirs.extend(rooted(root, caps[1].trim().trim_matches('"')).parent().map(Path::to_path_buf));
        }
        for caps in TEX_GRAPHICSPATH.captures_iter(&source) {
            for dir in BRACED.captures_iter(&caps[1]) {
                dirs.push(rooted(root, &dir[1]));
            }
        }
    }
    directories(dirs)
}

static TYPST_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\b(include|import|image|bibliography|read|csv|json|yaml|toml|xml|cbor|pdf)\s*\(?\s*(?:\w+\s*:\s*)?"([^"]+)""#,
    )
    .unwrap()
});

pub fn typst(main: &Path) -> Vec<PathBuf> {
    typst_text(main, None)
}

pub fn typst_text(main: &Path, main_text: Option<&str>) -> Vec<PathBuf> {
    let root = main.parent().unwrap_or(Path::new("."));
    let mut dirs = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = vec![main.to_path_buf()];

    while let Some(file) = queue.pop() {
        if visited.len() >= MAX_FILES || !visited.insert(file.clone()) {
            continue;
        }
        let Some(source) = read_source(&file, main, main_text) else { continue };
        let here = file.parent().unwrap_or(root);

        for caps in TYPST_PATH.captures_iter(&source) {
            let target = &caps[2];
            if target.starts_with('@') || is_remote(target) {
                continue;
            }
            let path = match target.strip_prefix('/') {
                Some(rest) => root.join(rest),
                None => here.join(target),
            };
            dirs.extend(path.parent().map(Path::to_path_buf));
            if matches!(&caps[1], "include" | "import") && path.extension().is_some_and(|e| e == "typ") {
                queue.push(path);
            }
        }
    }
    directories(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Project(PathBuf);

    impl Project {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("washi-deps-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, content).unwrap();
            path
        }

        fn dir(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sorted(mut v: Vec<PathBuf>) -> Vec<PathBuf> {
        v.sort();
        v
    }

    #[test]
    fn latex_finds_chapters_bibliography_and_figures() {
        let p = Project::new("tex");
        p.file("chapters/intro.tex", "intro");
        p.file("refs/refs.bib", "");
        p.file("figs/a.png", "");
        let main = p.file(
            "main.tex",
            "\\input{chapters/intro}\n\\bibliography{refs/refs}\n\\includegraphics[width=3cm]{figs/a.png}\n",
        );
        assert_eq!(
            sorted(latex(&main)),
            sorted(vec![p.dir("chapters"), p.dir("refs"), p.dir("figs")])
        );
    }

    #[test]
    fn latex_follows_nested_inputs_relative_to_the_main_file() {
        let p = Project::new("nested");
        p.file("a/one.tex", "\\input{b/two}");
        p.file("b/two.tex", "\\addbibresource{c/x.bib}");
        p.file("c/x.bib", "");
        let main = p.file("main.tex", "\\include{a/one}");
        assert_eq!(sorted(latex(&main)), sorted(vec![p.dir("a"), p.dir("b"), p.dir("c")]));
    }

    #[test]
    fn latex_ignores_comments_and_handles_lists_and_graphicspath() {
        let p = Project::new("misc");
        p.file("real/x.tex", "");
        p.file("gone/x.tex", "");
        p.file("img/a.png", "");
        p.file("refs/a.bib", "");
        p.file("refs/b.bib", "");
        let main = p.file(
            "main.tex",
            "% \\input{gone/x}\n\\input{real/x} 100\\% \\bibliography{refs/a, refs/b}\n\\graphicspath{{img/}}\n",
        );
        assert_eq!(sorted(latex(&main)), sorted(vec![p.dir("real"), p.dir("refs"), p.dir("img")]));
    }

    #[test]
    fn latex_uses_local_packages_only_when_they_exist() {
        let p = Project::new("sty");
        p.file("sty/mystyle.sty", "");
        let main = p.file("main.tex", "\\usepackage{amsmath}\n\\usepackage{sty/mystyle}\n");
        assert_eq!(latex(&main), vec![p.dir("sty")]);
    }

    #[test]
    fn latex_survives_cycles_and_missing_files() {
        let p = Project::new("cycle");
        p.file("a/a.tex", "\\input{b/b}");
        p.file("b/b.tex", "\\input{a/a}\n\\input{nowhere/x}");
        let main = p.file("main.tex", "\\input{a/a}");
        assert_eq!(sorted(latex(&main)), sorted(vec![p.dir("a"), p.dir("b")]));
    }

    #[test]
    fn a_dependency_in_the_documents_own_folder_is_reported_once() {
        let p = Project::new("flat");
        p.file("other.tex", "");
        let main = p.file("main.tex", "\\input{other}");
        assert_eq!(latex(&main), vec![p.0.clone()]);
    }

    #[test]
    fn typst_resolves_relative_paths_from_the_including_file() {
        let p = Project::new("typ");
        p.file("parts/one.typ", "#include \"deep/two.typ\"\n#image(\"pics/x.png\")");
        p.file("parts/deep/two.typ", "#bibliography(\"../../refs/r.bib\")");
        p.file("parts/pics/x.png", "");
        p.file("refs/r.bib", "");
        let main = p.file("main.typ", "#include \"parts/one.typ\"\n#import \"@preview/cetz:0.3.0\": canvas\n");
        assert_eq!(
            sorted(typst(&main)),
            sorted(vec![p.dir("parts"), p.dir("parts/deep"), p.dir("parts/pics"), p.dir("refs")])
        );
    }

    #[test]
    fn typst_slash_paths_start_at_the_project_root() {
        let p = Project::new("typroot");
        p.file("chapters/c.typ", "#image(\"/assets/logo.svg\")");
        p.file("assets/logo.svg", "");
        let main = p.file("main.typ", "#include \"chapters/c.typ\"");
        assert_eq!(sorted(typst(&main)), sorted(vec![p.dir("chapters"), p.dir("assets")]));
    }

    #[test]
    fn typst_reads_data_files_and_ignores_remote_and_packages() {
        let p = Project::new("typdata");
        p.file("data/t.csv", "");
        let main = p.file(
            "main.typ",
            "#let t = csv(\"data/t.csv\")\n#image(\"https://example.com/x.png\")\n#import \"@local/x:1.0.0\": y\n",
        );
        assert_eq!(typst(&main), vec![p.dir("data")]);
    }

    #[test]
    fn typst_survives_cycles() {
        let p = Project::new("typcycle");
        p.file("a/a.typ", "#include \"../b/b.typ\"");
        p.file("b/b.typ", "#include \"../a/a.typ\"");
        let main = p.file("main.typ", "#include \"a/a.typ\"");
        assert_eq!(sorted(typst(&main)), sorted(vec![p.dir("a"), p.dir("b")]));
    }

    #[test]
    fn markdown_watches_the_folders_of_local_images_only() {
        let p = Project::new("md");
        p.file("assets/a.png", "");
        let md = "![a](assets/a.png)\n![b](https://example.com/b.png)\n![c](data:image/png;base64,AAAA)\n[link](other.md)\n";
        assert_eq!(markdown(md, &p.0), vec![p.dir("assets")]);
    }

    #[test]
    fn broad_folders_are_never_watched() {
        assert!(directories(vec![PathBuf::from("/")]).is_empty());
        if let Some(home) = std::env::var_os("HOME") {
            assert!(directories(vec![PathBuf::from(home)]).is_empty());
        }
    }

    #[test]
    fn dot_segments_are_folded_and_duplicates_removed() {
        let p = Project::new("norm");
        p.file("a/x", "");
        let dirs = directories(vec![p.dir("a/../a"), p.dir("a"), p.dir("a/.")]);
        assert_eq!(dirs, vec![p.dir("a")]);
    }

    #[test]
    fn latex_scans_the_unsaved_text_instead_of_the_file_on_disk() {
        let p = Project::new("tex-buffer");
        p.file("chapters/one.tex", "");
        let main = p.file("main.tex", "nothing yet");
        assert!(latex(&main).is_empty() || latex(&main) == vec![p.0.clone()]);
        assert_eq!(latex_text(&main, Some("\\input{chapters/one}")), vec![p.dir("chapters")]);
    }

    #[test]
    fn typst_scans_the_unsaved_text_instead_of_the_file_on_disk() {
        let p = Project::new("typ-buffer");
        p.file("parts/a.typ", "");
        let main = p.file("main.typ", "= plain");
        assert_eq!(typst_text(&main, Some("#include \"parts/a.typ\"")), vec![p.dir("parts")]);
    }

    #[test]
    fn latex_follows_the_less_common_include_forms() {
        let forms = [
            ("\\input b/x", "b"),
            ("\\input{\"o/x\"}", "o"),
            ("\\InputIfFileExists{l/x}{}{}", "l"),
            ("\\import{e/}{x}", "e"),
            ("\\subimport{f/}{x}", "f"),
            ("\\includefrom{c/}{x}", "c"),
            ("\\subinputfrom{d/}{x}", "d"),
            ("\\inputminted[linenos]{python}{k/code.py}", "k"),
            ("\\includesvg[width=3cm]{g/pic}", "g"),
            ("\\addglobalbib{h/refs.bib}", "h"),
            ("\\addsectionbib[location=local]{i/refs.bib}", "i"),
        ];
        for (body, folder) in forms {
            let p = Project::new("forms");
            fs::create_dir_all(p.dir(folder)).unwrap();
            let main = p.file("main.tex", &format!("\\documentclass{{article}}\\begin{{document}}{body}\\end{{document}}"));
            assert!(latex(&main).contains(&p.dir(folder)), "{body} should watch {folder}: {:?}", latex(&main));
        }
    }

    #[test]
    fn latex_follows_a_file_reached_through_import() {
        let p = Project::new("import-chain");
        p.file("parts/a.tex", "\\input{figs/inner}");
        fs::create_dir_all(p.dir("figs")).unwrap();
        let main = p.file("main.tex", "\\subimport{parts/}{a}");
        let dirs = latex(&main);
        assert!(dirs.contains(&p.dir("parts")) && dirs.contains(&p.dir("figs")), "{dirs:?}");
    }

    #[test]
    fn latex_scans_files_that_are_not_utf8() {
        let p = Project::new("sjis");
        fs::create_dir_all(p.dir("inc")).unwrap();
        let mut bytes = b"\\documentclass{article}\\begin{document}".to_vec();
        bytes.extend_from_slice(&[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]);
        bytes.extend_from_slice(b"\\input{inc/a}\\end{document}");
        let main = p.0.join("main.tex");
        fs::write(&main, bytes).unwrap();
        assert!(latex(&main).contains(&p.dir("inc")), "a Shift_JIS main file is still scanned");
    }
}
