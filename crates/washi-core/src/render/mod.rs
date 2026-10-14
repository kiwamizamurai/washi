mod autolink;
mod deps;
mod detect;
pub use detect::extension_of_text;
mod markdown;
mod mermaid;
mod process;
mod latex;
mod latex_equations;
mod latex_marks;
pub mod tools;
mod offsets;
mod pdf_svg;
mod tikz;
mod typst;
mod typst_frames;

use std::path::{Path, PathBuf};

pub enum Output {
    Html(String),
}

impl Output {
    pub fn into_wire(self) -> Vec<u8> {
        let Self::Html(html) = self;
        let (tag, body) = (0u8, html.into_bytes());
        let mut wire = Vec::with_capacity(body.len() + 1);
        wire.push(tag);
        wire.extend_from_slice(&body);
        wire
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub severity: Severity,
    pub message: String,
    pub hints: Vec<String>,
}

pub struct Rendered {
    pub output: Result<Output, String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Rendered {
    pub fn into_wire(self) -> Vec<u8> {
        let diagnostics = serde_json::to_vec(&self.diagnostics).unwrap_or_else(|_| b"[]".to_vec());
        let (tag, body) = match self.output {
            Ok(Output::Html(html)) => (0u8, html.into_bytes()),
            Err(message) => (2u8, message.into_bytes()),
        };
        let mut wire = Vec::with_capacity(5 + diagnostics.len() + body.len());
        wire.push(tag);
        wire.extend_from_slice(&(diagnostics.len() as u32).to_be_bytes());
        wire.extend_from_slice(&diagnostics);
        wire.extend_from_slice(&body);
        wire
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub apply: Option<String>,
    pub detail: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Completions {
    pub offset: usize,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

pub trait Renderer: Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];
    fn render(&self, path: &Path) -> Result<Output, String>;

    fn render_text(&self, _source: &str) -> Result<Output, String> {
        Err("this format cannot be pasted".into())
    }

    fn dependency_dirs(&self, _path: &Path) -> Vec<PathBuf> {
        Vec::new()
    }

    fn render_buffer(&self, _path: &Path, text: &str) -> Rendered {
        Rendered { output: self.render_text(text), diagnostics: Vec::new() }
    }

    fn buffer_dependency_dirs(&self, path: &Path, _text: &str) -> Vec<PathBuf> {
        self.dependency_dirs(path)
    }

    fn complete(&self, _path: &Path, _text: &str, _offset: usize, _explicit: bool) -> Result<Completions, String> {
        Ok(Completions { offset: _offset, items: Vec::new() })
    }
}

static RENDERERS: [&dyn Renderer; 4] = [
    &markdown::MarkdownRenderer,
    &mermaid::MermaidRenderer,
    &typst::TypstRenderer,
    &latex::LatexRenderer { converter: latex::PandocConverter, tex: tikz::SystemTex },
];

pub fn renderer_for(path: &Path) -> Option<&'static dyn Renderer> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    RENDERERS
        .iter()
        .find(|r| r.extensions().contains(&ext.as_str()))
        .copied()
}

pub fn format_of(path: &Path) -> Option<&'static str> {
    renderer_for(path).map(|r| r.name())
}

pub fn formats() -> Vec<(&'static str, &'static [&'static str])> {
    RENDERERS.iter().map(|r| (r.name(), r.extensions())).collect()
}

pub fn supported_extensions() -> Vec<&'static str> {
    RENDERERS
        .iter()
        .flat_map(|r| r.extensions().iter().copied())
        .collect()
}

pub fn render(path: &Path) -> Result<Output, String> {
    renderer_for(path)
        .ok_or_else(|| format!("unsupported format: {}", path.display()))?
        .render(path)
}

pub fn dependency_dirs(path: &Path) -> Vec<PathBuf> {
    renderer_for(path).map(|r| r.dependency_dirs(path)).unwrap_or_default()
}

pub fn render_buffer(path: &Path, text: &str) -> Rendered {
    match renderer_for(path) {
        Some(renderer) => renderer.render_buffer(path, text),
        None => Rendered {
            output: Err(format!("unsupported format: {}", path.display())),
            diagnostics: Vec::new(),
        },
    }
}

pub fn buffer_dependency_dirs(path: &Path, text: &str) -> Vec<PathBuf> {
    renderer_for(path).map(|r| r.buffer_dependency_dirs(path, text)).unwrap_or_default()
}

pub fn complete(path: &Path, text: &str, offset: usize, explicit: bool) -> Result<Completions, String> {
    match renderer_for(path) {
        Some(renderer) => renderer.complete(path, text, offset, explicit),
        None => Ok(Completions { offset, items: Vec::new() }),
    }
}

pub fn render_text(text: &str) -> Result<Output, String> {
    let ext = detect::extension_of_text(text);
    renderer_for(Path::new(&format!("pasted.{ext}")))
        .ok_or("no renderer for this format")?
        .render_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_text_is_rendered_by_the_detected_format() {
        assert!(matches!(render_text("# Hello\n"), Ok(Output::Html(_))));
        assert!(matches!(render_text("graph LR; A-->B"), Ok(Output::Html(_))));
        assert!(matches!(render_text("#set page(paper: \"a4\")\n= Hi\n"), Ok(Output::Html(_))));
    }

    #[test]
    fn dispatches_by_extension_case_insensitively() {
        for name in ["a.md", "a.MARKDOWN", "a.typ", "a.tex", "a.mmd"] {
            assert!(renderer_for(Path::new(name)).is_some(), "{name}");
        }
        for name in ["a.txt", "a", "a.docx", "a.pdf"] {
            assert!(renderer_for(Path::new(name)).is_none(), "{name}");
        }
    }

    #[test]
    fn format_names_are_stable_and_unique() {
        let names: Vec<_> = formats().into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["markdown", "mermaid", "typst", "latex"]);
        assert_eq!(format_of(Path::new("/x/a.MD")), Some("markdown"));
        assert_eq!(format_of(Path::new("/x/a.latex")), Some("latex"));
        assert_eq!(format_of(Path::new("/x/a.txt")), None);
    }

    #[test]
    fn extensions_do_not_overlap() {
        let mut all = supported_extensions();
        let total = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), total);
    }

    #[test]
    fn wire_format_is_tag_then_body() {
        assert_eq!(Output::Html("x".into()).into_wire(), vec![0, b'x']);
    }

    #[test]
    fn unsupported_file_is_an_error() {
        assert!(render(Path::new("a.txt")).is_err());
    }
}

#[cfg(test)]
mod examples {
    use super::*;
    use std::path::PathBuf;

    fn example(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name)
    }

    fn html(name: &str) -> String {
        let Output::Html(h) = render(&example(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        h
    }

    #[test]
    fn showcase_markdown_uses_every_feature() {
        let h = html("showcase.md");
        for needle in [
            "<table", "data-sourcepos=\"", "data-math-style=\"display\"", "language-mermaid", "language-rust",
            "markdown-alert", "footnote", "data:image/svg+xml;base64,", "checkbox", "raw HTML omitted",
        ] {
            assert!(h.contains(needle), "showcase.md に {needle} が無い");
        }
        assert!(!h.contains("<script>"));
        assert!(!h.contains("author: kiwamizamurai"), "front matter が漏れている");
    }

    #[test]
    fn architecture_diagram_renders() {
        assert!(html("architecture.mmd").contains("classDiagram"));
    }

    #[test]
    fn report_typst_renders_headings_math_tables_and_the_bibliography() {
        let h = html("report.typ");
        for needle in ["<h2", "<math", "<table", "<figure", "<ol", "Washi Technical Report", "text-align: center", "font-size: 24pt", "align-self: center", "grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto minmax(0, 1fr)"] {
            assert!(h.contains(needle), "report.typ has no {needle}");
        }
        assert!(!h.contains("html export is under active development"));
    }

    #[test]
    #[ignore = "ネットワークから @preview パッケージを取得する"]
    fn packages_typst_shows_the_cetz_and_fletcher_drawings() {
        let h = html("packages.typ");
        assert!(h.contains("<h2"));
        assert_eq!(h.matches("class=\"washi-figure\"").count(), 2, "both drawings become figures");
        assert!(h.matches("<svg").count() >= 2);
    }
}

#[cfg(test)]
mod e2e_fixtures {
    use super::*;
    use std::{fs, path::PathBuf};

    #[test]
    #[ignore = "e2e/fixtures に実際の描画結果を書き出す"]
    fn dump() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out_dir = root.join("e2e/fixtures");
        fs::create_dir_all(&out_dir).unwrap();
        for name in ["showcase.md", "architecture.mmd", "report.typ", "paper.tex"] {
            let output = render(&root.join("examples").join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            fs::write(out_dir.join(format!("{name}.wire")), output.into_wire()).unwrap();
        }
    }

    #[test]
    fn a_buffer_wire_carries_the_tag_the_diagnostics_and_the_body() {
        fn split(wire: &[u8]) -> (u8, serde_json::Value, &[u8]) {
            let len = u32::from_be_bytes(wire[1..5].try_into().unwrap()) as usize;
            (wire[0], serde_json::from_slice(&wire[5..5 + len]).unwrap(), &wire[5 + len..])
        }
        let diagnostic = Diagnostic {
            file: None,
            line: 2,
            column: 3,
            end_line: 2,
            end_column: 9,
            severity: Severity::Error,
            message: "boom".into(),
            hints: vec!["try this".into()],
        };
        let ok = Rendered { output: Ok(Output::Html("<p>x</p>".into())), diagnostics: vec![] }.into_wire();
        assert_eq!(split(&ok), (0, serde_json::json!([]), "<p>x</p>".as_bytes()));
        let failed = Rendered { output: Err("失敗".into()), diagnostics: vec![diagnostic] }.into_wire();
        let (tag, json, body) = split(&failed);
        assert_eq!((tag, body), (2, "失敗".as_bytes()));
        assert_eq!(json[0]["line"], 2);
        assert_eq!(json[0]["severity"], "error");
        assert_eq!(json[0]["file"], serde_json::Value::Null);
        assert_eq!(json[0]["hints"][0], "try this");
    }

    #[test]
    fn unsupported_formats_have_no_buffer_render_or_completion() {
        let path = Path::new("/tmp/washi-none/a.txt");
        let rendered = render_buffer(path, "x");
        assert!(rendered.output.is_err() && rendered.diagnostics.is_empty());
        assert!(complete(path, "x", 1, false).unwrap().items.is_empty());
        assert!(buffer_dependency_dirs(path, "x").is_empty());
    }
}
