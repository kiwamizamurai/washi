use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::Engine as _;
use comrak::{format_html, nodes::NodeValue, parse_document, Arena, Options};

use super::{autolink, deps, Output, Rendered, Renderer};

const MAX_INLINE_IMAGE: u64 = 10 * 1024 * 1024;

pub struct MarkdownRenderer;

impl Renderer for MarkdownRenderer {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown", "mdown"]
    }

    fn render(&self, path: &Path) -> Result<Output, String> {
        let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        to_html_with(&source, Some(path.parent().unwrap_or(Path::new("."))), true).map(Output::Html)
    }

    fn render_text(&self, source: &str) -> Result<Output, String> {
        render_source(source, None)
    }

    fn dependency_dirs(&self, path: &Path) -> Vec<PathBuf> {
        let Ok(source) = fs::read_to_string(path) else { return Vec::new() };
        deps::markdown(&source, path.parent().unwrap_or(Path::new(".")))
    }

    fn render_buffer(&self, path: &Path, text: &str) -> Rendered {
        let output = to_html_with(text, Some(path.parent().unwrap_or(Path::new("."))), true).map(Output::Html);
        Rendered { output, diagnostics: Vec::new() }
    }

    fn buffer_dependency_dirs(&self, path: &Path, text: &str) -> Vec<PathBuf> {
        deps::markdown(text, path.parent().unwrap_or(Path::new(".")))
    }
}

pub(super) fn render_source(source: &str, base: Option<&Path>) -> Result<Output, String> {
    to_html(source, base).map(Output::Html)
}

fn options(sourcepos: bool) -> Options<'static> {
    let mut options = Options::default();
    options.render.sourcepos = sourcepos;
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;
    options.extension.math_dollars = true;
    options.extension.alerts = true;
    options.extension.superscript = true;
    options.extension.header_id_prefix = Some(String::new());
    options.extension.front_matter_delimiter = Some("---".into());
    options.render.r#unsafe = false;
    options
}

fn to_html(source: &str, base: Option<&Path>) -> Result<String, String> {
    to_html_with(source, base, false)
}

fn to_html_with(source: &str, base: Option<&Path>, sourcepos: bool) -> Result<String, String> {
    let options = options(sourcepos);
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options);
    autolink::link_ascii_urls(&arena, root);

    let mut inlined: Vec<String> = Vec::new();
    if let Some(base) = base {
        for node in root.descendants() {
            if let NodeValue::Image(link) = &mut node.data.borrow_mut().value {
                if let Some(uri) = inline_local_image(base, &link.url) {
                    link.url = placeholder(inlined.len());
                    inlined.push(uri);
                }
            }
        }
    }

    let mut html = String::new();
    format_html(root, &options, &mut html).map_err(|e| e.to_string())?;
    for (index, uri) in inlined.iter().enumerate() {
        html = html.replace(&placeholder(index), uri);
    }
    Ok(html)
}

fn placeholder(index: usize) -> String {
    format!("washi-inline-image:{index}.")
}

fn inline_local_image(base: &Path, url: &str) -> Option<String> {
    if url.contains("://") || url.starts_with("data:") {
        return None;
    }
    let path = base.join(url.split(['#', '?']).next()?);
    let meta = fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_INLINE_IMAGE {
        return None;
    }
    let mime = match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(fs::read(&path).ok()?);
    Some(format!("data:{mime};base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(source: &str, base: &Path) -> String {
        to_html(source, Some(base)).unwrap()
    }

    #[test]
    fn pasted_text_never_reads_local_files() {
        let out = to_html("![](Cargo.toml)\n", None).unwrap();
        assert!(out.contains("Cargo.toml"), "{out}");
        assert!(!out.contains("data:"), "{out}");
    }

    #[test]
    fn renders_headings_math_tables_and_tasks() {
        let out = html(
            "# 見出し\n\n$e^{i\\pi}=-1$\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n",
            Path::new("."),
        );
        assert!(out.contains("<h1"), "{out}");
        assert!(out.contains("data-math-style"), "{out}");
        assert!(out.contains("<table>"), "{out}");
        assert!(out.contains("checkbox"), "{out}");
    }

    #[test]
    fn currency_is_not_math() {
        let out = html("It costs $5 and then $10 more.\n", Path::new("."));
        assert!(!out.contains("data-math-style"), "{out}");
    }

    #[test]
    fn inlines_svg_images_which_the_safe_renderer_would_otherwise_blank() {
        let dir = std::env::temp_dir().join(format!("washi-svg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let out = html("![a](a.svg)\n", &dir);
        assert!(out.contains("data:image/svg+xml;base64,"), "{out}");
    }

    #[test]
    fn url_followed_by_japanese_does_not_swallow_the_rest() {
        let out = html(
            "詳細は https://example.com、脚注[^a] を参照。\n\n[^a]: 補足\n",
            Path::new("."),
        );
        assert!(out.contains("<a href=\"https://example.com\">https://example.com</a>、"), "{out}");
        assert!(out.contains("data-footnote-ref"), "{out}");
        assert!(out.contains("data-footnotes"), "{out}");
    }

    #[test]
    fn urls_inside_code_and_explicit_links_are_left_alone() {
        let out = html("`https://a.com` [t](https://b.com) <https://c.com>\n", Path::new("."));
        assert!(out.contains("<code>https://a.com</code>"), "{out}");
        assert_eq!(out.matches("<a ").count(), 2, "{out}");
    }

    #[test]
    fn strips_front_matter() {
        let out = html("---\ntitle: x\n---\n# a\n", Path::new("."));
        assert!(!out.contains("title: x"), "{out}");
    }

    #[test]
    fn escapes_raw_html() {
        let out = html("<script>alert(1)</script>\n", Path::new("."));
        assert!(!out.contains("<script>"), "{out}");
    }

    #[test]
    fn inlines_relative_images_only() {
        let dir = std::env::temp_dir().join(format!("washi-md-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let out = html(
            "![](a.png)\n\n![](https://example.com/b.png)\n\n![](missing.png)\n",
            &dir,
        );
        assert!(out.contains("data:image/png;base64,"), "{out}");
        assert!(!out.contains("washi-inline-image"), "{out}");
        assert!(out.contains("https://example.com/b.png"), "{out}");
        assert!(out.contains("missing.png"), "{out}");
    }

    fn project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-md-buf-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("assets")).unwrap();
        fs::write(dir.join("assets/a.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        dir
    }

    fn html_of(rendered: Rendered) -> String {
        match rendered.output.expect("描画できる") {
            Output::Html(h) => h,
        }
    }

    #[test]
    fn a_buffer_embeds_relative_images_like_the_saved_file() {
        let dir = project("img");
        let path = dir.join("doc.md");
        let out = html_of(MarkdownRenderer.render_buffer(&path, "![x](assets/a.svg)\n"));
        assert!(out.contains("data:image/svg+xml;base64,"), "{out}");
    }

    fn html_of_text(text: &str) -> String {
        match MarkdownRenderer.render_text(text).unwrap() {
            Output::Html(h) => h,
        }
    }

    #[test]
    fn both_the_saved_file_and_a_buffer_mark_elements_with_their_source_lines() {
        let dir = project("pos");
        let path = dir.join("doc.md");
        let text = "# Title\n\nfirst\n\nsecond\n";
        let buffer = html_of(MarkdownRenderer.render_buffer(&path, text));
        assert!(buffer.contains("data-sourcepos=\"1:1-1:7\""), "{buffer}");
        assert!(buffer.contains("data-sourcepos=\"5:1-5:6\""), "{buffer}");
        fs::write(&path, text).unwrap();
        let saved = match MarkdownRenderer.render(&path).unwrap() {
            Output::Html(h) => h,
        };
        assert!(saved.contains("data-sourcepos=\"1:1-1:7\""), "reading mode can jump to the source too: {saved}");
        assert!(!html_of_text("# Title\n").contains("data-sourcepos"), "pasted text has no file to jump to");
    }

    #[test]
    fn a_buffer_renders_exactly_like_the_saved_file() {
        let dir = project("same");
        let path = dir.join("doc.md");
        let text = "# Hi\n\n$x^2$ and `code`\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        fs::write(&path, text).unwrap();
        let buffer = html_of(MarkdownRenderer.render_buffer(&path, text));
        let saved = match MarkdownRenderer.render(&path).unwrap() {
            Output::Html(h) => h,
        };
        assert_eq!(buffer, saved);
    }

    #[test]
    fn buffer_dependency_dirs_follow_the_unsaved_text() {
        let dir = project("deps");
        let path = dir.join("doc.md");
        assert_eq!(MarkdownRenderer.buffer_dependency_dirs(&path, "![x](assets/a.svg)"), vec![dir.join("assets")]);
        assert!(MarkdownRenderer.buffer_dependency_dirs(&path, "no images").is_empty());
    }
}
