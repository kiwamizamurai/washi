use std::{
    collections::HashMap,
    fs,
    ops::Range,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use typst::{
    diag::{FileResult, Severity, SourceDiagnostic},
    foundations::{Bytes, Datetime, Duration},
    syntax::{DiagSpan, DiagSpanKind, FileId, Source, Span, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
    Library, World, WorldExt,
};
use typst_as_lib::{typst_kit_options::TypstKitFontOptions, TypstEngine};
use typst_html::{HtmlAttr, HtmlDocument, HtmlElement, HtmlNode};
use typst_ide::IdeWorld;

use super::{
    deps, offsets,
    typst_frames::{self, Edits, Wrap}, CompletionItem, Completions, Diagnostic, Output, Rendered, Renderer, Severity as DiagSeverity,
};

const HTML_EXPORT_NOTICE: &str = "html export is under active development";
static SOURCEPOS: LazyLock<HtmlAttr> = LazyLock::new(|| HtmlAttr::intern("data-sourcepos").expect("a valid attribute name"));
static SOURCE_FILE: LazyLock<HtmlAttr> = LazyLock::new(|| HtmlAttr::intern("data-source-file").expect("a valid attribute name"));

pub struct TypstRenderer;

impl Renderer for TypstRenderer {
    fn name(&self) -> &'static str {
        "typst"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["typ"]
    }

    fn render(&self, path: &Path) -> Result<Output, String> {
        let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        compile_html(source, &root).0.map(Output::Html)
    }

    fn render_text(&self, source: &str) -> Result<Output, String> {
        compile_html(source.to_owned(), &std::env::temp_dir()).0.map(Output::Html)
    }

    fn render_buffer(&self, path: &Path, text: &str) -> Rendered {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let (result, diagnostics) = compile_html(text.to_owned(), &root);
        Rendered { output: result.map(Output::Html), diagnostics }
    }

    fn buffer_dependency_dirs(&self, path: &Path, text: &str) -> Vec<PathBuf> {
        deps::typst_text(path, Some(text))
    }

    fn complete(&self, path: &Path, text: &str, offset: usize, explicit: bool) -> Result<Completions, String> {
        let cursor = offsets::utf16_to_byte(text, offset).ok_or("the position is inside a character")?;
        let root = path.parent().unwrap_or(Path::new("."));
        let engine = TypstEngine::builder()
            .main_file(text.to_owned())
            .search_fonts_with(
                TypstKitFontOptions::default()
                    .include_system_fonts(false)
                    .include_embedded_fonts(true),
            )
            .with_file_system_resolver(root)
            .build();
        let found = engine
            .with_world(|world| {
                let source = world.source(world.main()).ok()?;
                typst_ide::autocomplete(&IdeAdapter(world), None::<&HtmlDocument>, &source, cursor, explicit)
            })
            .map_err(|e| format!("cannot initialise Typst: {e}"))?;
        let Some((start, items)) = found else {
            return Ok(Completions { offset, items: Vec::new() });
        };
        Ok(Completions {
            offset: offsets::byte_to_utf16(text, start),
            items: items.into_iter().map(completion_item).collect(),
        })
    }

    fn dependency_dirs(&self, path: &Path) -> Vec<PathBuf> {
        super::deps::typst(path)
    }
}

fn completion_item(c: typst_ide::Completion) -> CompletionItem {
    use typst_ide::CompletionKind as K;
    let kind = match c.kind {
        K::Syntax => "syntax",
        K::Func => "func",
        K::Type => "type",
        K::Param => "param",
        K::Constant => "constant",
        K::Path => "path",
        K::Package => "package",
        K::Label => "label",
        K::Font => "font",
        K::Symbol(_) => "symbol",
    };
    CompletionItem {
        label: c.label.to_string(),
        apply: c.apply.map(|a| a.to_string()),
        detail: c.detail.map(|d| d.to_string()),
        kind: kind.to_owned(),
    }
}

struct IdeAdapter<'a>(&'a dyn World);

impl World for IdeAdapter<'_> {
    fn library(&self) -> &LazyHash<Library> {
        self.0.library()
    }
    fn book(&self) -> &LazyHash<FontBook> {
        self.0.book()
    }
    fn main(&self) -> FileId {
        self.0.main()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        self.0.source(id)
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.0.file(id)
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.0.font(index)
    }
    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.0.today(offset)
    }
}

impl IdeWorld for IdeAdapter<'_> {
    fn upcast(&self) -> &dyn World {
        self
    }
}

const MAX_RETRIES: usize = 4;

#[derive(Clone, Default)]
struct Plan {
    frames: HashMap<String, Vec<Wrap>>,
    disabled: Vec<(String, Range<usize>)>,
}

impl Plan {
    fn wraps_for(&self, key: &str, original: &str) -> Vec<Wrap> {
        typst_frames::styled_text_wraps(original)
            .into_iter()
            .chain(self.frames.get(key).cloned().unwrap_or_default())
            .filter(|w| !self.disabled.iter().any(|(k, r)| k == key && *r == w.range))
            .collect()
    }

    fn disable(&mut self, bad: &[(String, Range<usize>)]) {
        self.disabled.extend(bad.iter().cloned());
    }
}

const MAIN_KEY: &str = "";

struct Included {
    vpath: String,
    text: String,
}

struct Attempt {
    result: Result<String, String>,
    diagnostics: Vec<Diagnostic>,
    retry: Option<HashMap<String, Vec<Wrap>>>,
    failed_wraps: Vec<(String, Range<usize>)>,
}

fn compile_html(source: String, root: &Path) -> (Result<String, String>, Vec<Diagnostic>) {
    let included = included_files(&source, root);
    let mut plan = Plan::default();
    let mut fallback: Option<Attempt> = None;
    let mut last = None;
    for _ in 0..=MAX_RETRIES {
        let attempt = attempt(&source, root, &included, &plan);
        if attempt.result.is_err() {
            if attempt.failed_wraps.is_empty() {
                last = Some(attempt);
                break;
            }
            plan.disable(&attempt.failed_wraps);
            last = Some(attempt);
            continue;
        }
        match attempt.retry.clone() {
            Some(frames) if plan.frames.is_empty() && frames.values().any(|w| !w.is_empty()) => {
                plan.frames = frames;
                fallback = Some(attempt);
            }
            _ => return (attempt.result, attempt.diagnostics),
        }
    }
    let chosen = fallback.or(last).expect("at least one attempt ran");
    (chosen.result, chosen.diagnostics)
}

fn included_files(source: &str, root: &Path) -> Vec<Included> {
    let mut found: Vec<Included> = Vec::new();
    let mut queue: Vec<(String, String)> = typst_frames::include_paths(source)
        .into_iter()
        .map(|p| (String::from("/"), p))
        .collect();
    while let Some((dir, path)) = queue.pop() {
        let vpath = join_vpath(&dir, &path);
        if found.iter().any(|f| f.vpath == vpath) || found.len() >= 64 {
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(vpath.trim_start_matches('/'))) else { continue };
        let child_dir = vpath.rsplit_once('/').map_or("/".to_owned(), |(d, _)| format!("{d}/"));
        queue.extend(typst_frames::include_paths(&text).into_iter().map(|p| (child_dir.clone(), p)));
        found.push(Included { vpath, text });
    }
    found
}

fn join_vpath(dir: &str, path: &str) -> String {
    let joined = if path.starts_with('/') { path.to_owned() } else { format!("{dir}{path}") };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

struct FileEdit {
    original: String,
    edits: Edits,
    key: String,
}

struct Positions {
    main: FileId,
    main_edit: FileEdit,
    includes: HashMap<String, FileEdit>,
}

impl Positions {
    fn edit_for(&self, id: FileId) -> Option<&FileEdit> {
        if id == self.main {
            return Some(&self.main_edit);
        }
        if !matches!(id.root(), VirtualRoot::Project) {
            return None;
        }
        self.includes.get(&id.vpath().get_with_slash().to_string())
    }

    fn original_range(&self, world: &dyn World, span: DiagSpan) -> Option<(FileId, Range<usize>)> {
        let id = match span.get() {
            DiagSpanKind::Number { id, .. } | DiagSpanKind::Range { id, .. } => id,
            DiagSpanKind::Detached => return None,
        };
        let range = world.range(span)?;
        let Some(edit) = self.edit_for(id) else {
            return Some((id, range));
        };
        if let Some(start) = edit.edits.to_original(range.start) {
            let end = if range.end > range.start {
                edit.edits.to_original(range.end - 1).map_or(start, |e| e + 1)
            } else {
                start
            };
            return Some((id, start..end.max(start)));
        }
        edit.edits.wrapped_call_at(range.start).map(|call| (id, call))
    }

    fn place(&self, world: &dyn World, span: DiagSpan) -> Option<Place> {
        let (id, range) = self.original_range(world, span)?;
        let owned;
        let text = match self.edit_for(id) {
            Some(edit) => edit.original.as_str(),
            None => {
                owned = world.source(id).ok()?.text().to_owned();
                owned.as_str()
            }
        };
        let (line, column) = offsets::byte_to_line_column(text, range.start);
        let (end_line, end_column) = offsets::byte_to_line_column(text, range.end);
        let file = (id != self.main).then(|| id.vpath().get_without_slash().to_owned());
        Some(Place { file, line, column, end_line, end_column })
    }

    fn key_of(&self, id: FileId) -> Option<String> {
        self.edit_for(id).map(|e| e.key.clone())
    }
}

fn attempt(source: &str, root: &Path, included: &[Included], plan: &Plan) -> Attempt {
    let main_wraps = plan.wraps_for(MAIN_KEY, source);
    let main = typst_frames::rewrite(source, Some(typst_frames::PRELUDE), &main_wraps);
    let mut applied: HashMap<String, Vec<Wrap>> = HashMap::from([(MAIN_KEY.to_owned(), main_wraps)]);
    let mut includes: HashMap<String, FileEdit> = HashMap::new();
    let mut overlays: Vec<(String, String)> = Vec::new();
    for file in included {
        let wraps = plan.wraps_for(&file.vpath, &file.text);
        let rewritten = typst_frames::rewrite(&file.text, None, &wraps);
        applied.insert(file.vpath.clone(), wraps);
        if rewritten.text != file.text {
            overlays.push((file.vpath.clone(), rewritten.text));
        }
        includes.insert(
            file.vpath.clone(),
            FileEdit { original: file.text.clone(), edits: rewritten.edits, key: file.vpath.clone() },
        );
    }
    let engine = TypstEngine::builder()
        .main_file(main.text)
        .search_fonts_with(
            TypstKitFontOptions::default()
                .include_system_fonts(true)
                .include_embedded_fonts(true),
        )
        .with_static_source_file_resolver(overlays.iter().map(|(vpath, text)| (vpath.as_str(), text.clone())))
        .with_file_system_resolver(root)
        .with_package_file_resolver()
        .build();

    let main_edit = FileEdit { original: source.to_owned(), edits: main.edits, key: MAIN_KEY.to_owned() };
    let mut outcome = Attempt { result: Err(String::new()), diagnostics: Vec::new(), retry: None, failed_wraps: Vec::new() };
    let ran = engine.with_world(|world| {
        let positions = Positions { main: world.main(), main_edit, includes };
        let warned = typst::compile::<HtmlDocument>(world);
        let significant = warned.warnings.iter().any(|w| typst_frames::is_significant_drop(&w.message));
        let shown: Vec<SourceDiagnostic> = warned
            .warnings
            .iter()
            .filter(|w| !w.message.contains(HTML_EXPORT_NOTICE) && !typst_frames::is_harmless_drop(&w.message))
            .cloned()
            .collect();
        outcome.diagnostics.extend(diagnostics_of(world, &positions, &shown));
        let mut document = match warned.output {
            Ok(document) => document,
            Err(errors) => {
                outcome.failed_wraps = wraps_hit_by(world, &positions, &applied, &errors);
                outcome.diagnostics.extend(diagnostics_of(world, &positions, &errors));
                outcome.result = Err(describe_all(world, &positions, &errors));
                return;
            }
        };
        if plan.frames.is_empty() && significant {
            outcome.retry = Some(plan_for_dropped(world, &positions, &document, &warned.warnings));
        }
        stamp_sources(world, &positions, document.root_mut());
        outcome.result = typst_html::html(&document, &Default::default())
            .map(|html| html_fragment(&html))
            .map_err(|errors| {
                outcome.diagnostics.extend(diagnostics_of(world, &positions, &errors));
                describe_all(world, &positions, &errors)
            });
    });
    if let Err(e) = ran {
        outcome.result = Err(format!("cannot initialise Typst: {e}"));
    }
    outcome
}

fn wraps_hit_by(world: &dyn World, positions: &Positions, applied: &HashMap<String, Vec<Wrap>>, errors: &[SourceDiagnostic]) -> Vec<(String, Range<usize>)> {
    let mut hit = Vec::new();
    for error in errors {
        let spans = std::iter::once(error.span).chain(error.trace.iter().map(|t| DiagSpan::from(t.span)));
        for span in spans {
            let Some((id, range)) = positions.original_range(world, span) else { continue };
            let Some(key) = positions.key_of(id) else { continue };
            for wrap in applied.get(&key).map_or(&[][..], Vec::as_slice) {
                if wrap.range.start <= range.start && range.start <= wrap.range.end {
                    hit.push((key.clone(), wrap.range.clone()));
                }
            }
        }
    }
    hit
}

fn plan_for_dropped(world: &dyn World, positions: &Positions, document: &HtmlDocument, warnings: &[SourceDiagnostic]) -> HashMap<String, Vec<Wrap>> {
    let mut covered: HashMap<String, Vec<Range<usize>>> = HashMap::new();
    collect_coverage(world, positions, document.root(), &mut covered);
    let mut drops: HashMap<String, Vec<Range<usize>>> = HashMap::new();
    let mut drop_files: Vec<String> = Vec::new();
    for warning in warnings.iter().filter(|w| typst_frames::is_significant_drop(&w.message)) {
        let Some((id, range)) = positions.original_range(world, warning.span) else { continue };
        let key = positions.key_of(id).unwrap_or_else(|| format!("{id:?}"));
        drop_files.push(key.clone());
        drops.entry(key).or_default().push(range);
    }
    let global = world.library().global.scope();
    let is_builtin = |name: &str| global.get(name).is_some();
    let mut plan: HashMap<String, Vec<Wrap>> = HashMap::new();
    let keys = std::iter::once(MAIN_KEY.to_owned()).chain(positions.includes.keys().cloned());
    for key in keys {
        let original = if key == MAIN_KEY {
            positions.main_edit.original.as_str()
        } else {
            positions.includes[&key].original.as_str()
        };
        let evidence = typst_frames::Evidence {
            covered: covered.remove(&key).unwrap_or_default(),
            drops_here: drops.get(&key).cloned().unwrap_or_default(),
            drops_elsewhere: drop_files.iter().any(|f| *f != key),
        };
        let wraps = typst_frames::find_dropped_calls(original, &evidence, &is_builtin);
        plan.insert(key, wraps);
    }
    plan
}

fn collect_coverage(world: &dyn World, positions: &Positions, element: &HtmlElement, covered: &mut HashMap<String, Vec<Range<usize>>>) {
    note_coverage(world, positions, element.span, covered);
    for child in &element.children {
        match child {
            HtmlNode::Element(inner) => collect_coverage(world, positions, inner, covered),
            HtmlNode::Text(_, span) => note_coverage(world, positions, *span, covered),
            HtmlNode::Frame(frame) => note_coverage(world, positions, frame.span, covered),
            HtmlNode::Tag(_) => {}
        }
    }
}

fn note_coverage(world: &dyn World, positions: &Positions, span: Span, covered: &mut HashMap<String, Vec<Range<usize>>>) {
    if let Some((id, range)) = positions.original_range(world, DiagSpan::from(span)) {
        if let Some(key) = positions.key_of(id) {
            covered.entry(key).or_default().push(range);
        }
    }
}

fn stamp_sources(world: &dyn World, positions: &Positions, element: &mut HtmlElement) {
    if let Some(place) = positions.place(world, DiagSpan::from(element.span)) {
        let position = format!("{}:{}-{}:{}", place.line, place.column, place.end_line, place.end_column);
        match place.file {
            None => element.attrs.push(*SOURCEPOS, position),
            Some(file) => {
                element.attrs.push(*SOURCE_FILE, file);
                element.attrs.push(*SOURCEPOS, position);
            }
        }
    }
    for child in element.children.make_mut() {
        if let HtmlNode::Element(inner) = child {
            stamp_sources(world, positions, inner);
        }
    }
}

struct Place {
    file: Option<String>,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
}

fn html_fragment(document: &str) -> String {
    let styles = tagged_blocks(document, "style");
    let body = match document.find("<body") {
        Some(start) => {
            let open_end = document[start..].find('>').map_or(start, |i| start + i + 1);
            let close = document.rfind("</body>").unwrap_or(document.len());
            if open_end <= close { &document[open_end..close] } else { "" }
        }
        None => document,
    };
    format!("{styles}{body}")
}

fn tagged_blocks(document: &str, tag: &str) -> String {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = String::new();
    let mut rest = document;
    while let Some(start) = rest.find(&open) {
        let Some(end) = rest[start..].find(&close) else { break };
        let stop = start + end + close.len();
        out.push_str(&rest[start..stop]);
        rest = &rest[stop..];
    }
    out
}

fn diagnostics_of(world: &dyn World, positions: &Positions, items: &[SourceDiagnostic]) -> Vec<Diagnostic> {
    items
        .iter()
        .map(|d| {
            let severity = match d.severity {
                Severity::Error => DiagSeverity::Error,
                Severity::Warning => DiagSeverity::Warning,
            };
            let place = positions.place(world, d.span);
            let (file, line, column, end_line, end_column) = place
                .map(|p| (p.file, p.line, p.column, p.end_line, p.end_column))
                .unwrap_or((None, 1, 1, 1, 1));
            Diagnostic {
                file,
                line,
                column,
                end_line,
                end_column,
                severity,
                message: d.message.to_string(),
                hints: d.hints.iter().map(|h| h.v.to_string()).collect(),
            }
        })
        .collect()
}

fn describe_all(world: &dyn World, positions: &Positions, diagnostics: &[SourceDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| describe(world, positions, d))
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe(world: &dyn World, positions: &Positions, diagnostic: &SourceDiagnostic) -> String {
    let level = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let location = positions
        .place(world, diagnostic.span)
        .map(|p| format!("{}:{}:{}: ", p.file.as_deref().unwrap_or("main.typ"), p.line, p.column))
        .unwrap_or_default();
    let mut text = format!("{location}{level}: {}", diagnostic.message);
    for hint in &diagnostic.hints {
        text.push_str(&format!("\n  hint: {}", hint.v));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_of(source: &str) -> String {
        let Output::Html(html) = TypstRenderer.render_text(source).expect("compiles");
        html
    }

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("washi-typ-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compiles_headings_math_and_tables_to_html() {
        let html = html_of("= Title\n#lorem(10)\n$ sum_(i=1)^n i = (n(n+1))/2 $\n#table(columns: 2, [a], [b])\n");
        assert!(html.contains("<h2"), "{html}");
        assert!(html.contains("<math"), "{html}");
        assert!(html.contains("<table"), "{html}");
    }

    #[test]
    fn renders_a_saved_file_the_same_as_its_buffer() {
        let dir = temp_project("same");
        let main = dir.join("main.typ");
        let text = "= Hello\nSome text with $x^2$.\n";
        fs::write(&main, text).unwrap();
        let Output::Html(saved) = TypstRenderer.render(&main).unwrap();
        let Output::Html(buffer) = TypstRenderer.render_buffer(&main, text).output.unwrap();
        assert_eq!(buffer, saved);
    }

    #[test]
    fn keeps_only_styles_and_body_of_the_full_document() {
        let full = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>T</title><style>a{b:c}</style></head><body><p>Hi</p></body></html>";
        assert_eq!(html_fragment(full), "<style>a{b:c}</style><p>Hi</p>");
    }

    #[test]
    fn a_fragment_without_a_body_is_kept_whole() {
        assert_eq!(html_fragment("<p>Hi</p>"), "<p>Hi</p>");
    }

    #[test]
    fn the_notice_that_html_export_is_unfinished_is_not_shown_as_a_diagnostic() {
        let dir = temp_project("notice");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "= ok\n");
        assert!(rendered.diagnostics.iter().all(|d| !d.message.contains(HTML_EXPORT_NOTICE)), "{:?}", rendered.diagnostics);
    }

    #[test]
    fn a_buffer_resolves_includes_from_the_documents_folder() {
        let dir = temp_project("buf-include");
        fs::write(dir.join("part.typ"), "Part text.\n").unwrap();
        let main = dir.join("main.typ");
        let rendered = TypstRenderer.render_buffer(&main, "= Title\n#include \"part.typ\"\n");
        assert!(rendered.output.is_ok(), "{:?}", rendered.output.err());
        assert!(rendered.diagnostics.iter().all(|d| d.severity != DiagSeverity::Error), "{:?}", rendered.diagnostics);
    }

    #[test]
    fn errors_come_back_as_structured_diagnostics_even_though_the_render_fails() {
        let dir = temp_project("buf-error");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "= A\n#undefined-fn()\n");
        assert!(rendered.output.is_err());
        let error = rendered.diagnostics.iter().find(|d| d.severity == DiagSeverity::Error).expect("an error diagnostic");
        assert_eq!((error.file.as_deref(), error.line), (None, 2));
        assert!(error.column >= 1 && error.end_column >= error.column, "{error:?}");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn diagnostic_columns_count_code_points_not_bytes() {
        let dir = temp_project("buf-cols");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "日本😀 #undefined-fn()\n");
        let error = rendered.diagnostics.iter().find(|d| d.severity == DiagSeverity::Error).expect("an error diagnostic");
        assert_eq!(error.line, 1);
        assert_eq!(error.column, 6, "{error:?}");
    }

    #[test]
    fn warnings_are_reported_while_the_render_succeeds() {
        let dir = temp_project("buf-warn");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "#set text(font: \"No Such Font Family Washi\")\nHello\n");
        assert!(rendered.output.is_ok(), "{:?}", rendered.output.err());
        assert!(rendered.diagnostics.iter().any(|d| d.severity == DiagSeverity::Warning), "{:?}", rendered.diagnostics);
    }

    #[test]
    fn a_diagnostic_in_an_included_file_names_that_file() {
        let dir = temp_project("buf-other-file");
        fs::write(dir.join("bad.typ"), "#undefined-in-part()\n").unwrap();
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "#include \"bad.typ\"\n");
        let error = rendered.diagnostics.iter().find(|d| d.severity == DiagSeverity::Error).expect("an error diagnostic");
        assert_eq!(error.file.as_deref(), Some("bad.typ"), "{error:?}");
    }

    #[test]
    fn completion_offers_functions_and_reports_utf16_offsets() {
        let dir = temp_project("buf-complete");
        let path = dir.join("main.typ");
        let plain = TypstRenderer.complete(&path, "#lore", 5, false).unwrap();
        assert!(plain.items.iter().any(|c| c.label == "lorem" && c.kind == "func"), "{:?}", plain.items.iter().map(|c| &c.label).collect::<Vec<_>>());

        let text = "日本語😀\n#lore";
        let cursor = text.encode_utf16().count();
        let after = TypstRenderer.complete(&path, text, cursor, false).unwrap();
        assert!(after.items.iter().any(|c| c.label == "lorem"));
        let start = "日本語😀\n#".encode_utf16().count();
        assert_eq!(after.offset, start, "the replacement starts at the head of `lore`");
    }

    #[test]
    fn completion_rejects_a_position_inside_a_surrogate_pair() {
        let path = Path::new("/tmp/washi-none/main.typ");
        assert!(TypstRenderer.complete(path, "a😀b", 2, false).is_err());
    }

    #[test]
    fn buffer_dependency_dirs_follow_the_unsaved_text() {
        let dir = temp_project("buf-deps");
        fs::create_dir_all(dir.join("parts")).unwrap();
        let main = dir.join("main.typ");
        assert_eq!(TypstRenderer.buffer_dependency_dirs(&main, "#include \"parts/a.typ\""), vec![dir.join("parts")]);
    }

    #[test]
    fn elements_carry_the_source_position_they_came_from() {
        let html = html_of("= Title\n\nA paragraph of text.\n");
        assert!(html.contains("data-sourcepos=\"1:"), "the heading is on line 1: {html}");
        assert!(html.contains("data-sourcepos=\"3:"), "the paragraph is on line 3: {html}");
        assert!(!html.contains("data-source-file"), "the main file needs no file name: {html}");
    }

    #[test]
    fn elements_from_an_included_file_name_that_file() {
        let dir = temp_project("sourcepos-include");
        fs::write(dir.join("part.typ"), "== Part\n\nFrom the part.\n").unwrap();
        let main = dir.join("main.typ");
        fs::write(&main, "= Main\n\n#include \"part.typ\"\n").unwrap();
        let Output::Html(html) = TypstRenderer.render(&main).unwrap();
        assert!(html.contains("data-source-file=\"part.typ\""), "{html}");
        assert!(html.contains("data-sourcepos=\"1:"), "{html}");
    }

    const DRAWING: &str = "#let drawing() = context layout(_ => box(width: 2cm, height: 1cm, place(rect(width: 1cm, height: 5mm))))\n";

    #[test]
    fn a_title_inside_align_is_kept_and_centered() {
        let html = html_of("#align(center)[*Big Title*]\n");
        assert!(html.contains("text-align: center"), "{html}");
        assert!(html.contains("Big Title"), "{html}");
    }

    #[test]
    fn a_title_keeps_its_size_and_weight() {
        let html = html_of("#align(center)[#text(size: 24pt, weight: \"bold\")[Big Title]]\n");
        assert!(html.contains("font-size: 24pt; font-weight: 700"), "{html}");
        assert!(html.contains("Big Title"), "{html}");
    }

    #[test]
    fn a_grid_keeps_its_column_widths_and_alignment() {
        let html = html_of("#grid(columns: (1fr, auto, 1fr), align: horizon + center, [A], [→], [B])\n");
        assert!(html.contains("grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr)"), "{html}");
        assert_eq!(html.matches("align-self: center").count(), 3, "{html}");
    }

    #[test]
    fn grids_and_columns_keep_their_text() {
        let html = html_of("#grid(columns: (1fr, 1fr), [Left cell], [Right cell])\n#columns(2)[Column text]\n");
        assert!(html.contains("Left cell") && html.contains("Right cell"), "{html}");
        assert!(html.contains("column-count: 2") && html.contains("Column text"), "{html}");
    }

    #[test]
    fn a_drawing_that_html_cannot_show_becomes_an_svg_figure() {
        let html = html_of(&format!("{DRAWING}\n#drawing()\n"));
        assert!(html.contains("washi-figure"), "{html}");
        assert!(html.contains("<svg"), "{html}");
    }

    #[test]
    fn a_figure_keeps_its_caption_around_the_drawing() {
        let html = html_of(&format!("{DRAWING}\n#figure(drawing(), caption: [Shown below])\n"));
        assert!(html.contains("<figure"), "{html}");
        assert!(html.contains("<svg"), "{html}");
        assert!(html.contains("Shown below"), "{html}");
    }

    #[test]
    fn positions_around_a_wrapped_drawing_point_at_the_original_lines() {
        let html = html_of(&format!("{DRAWING}\n#drawing() Some words.\n\nNext paragraph.\n"));
        assert!(html.contains("data-sourcepos=\"3:2-3:11\""), "the figure points at the call: {html}");
        let next = html.find("Next paragraph").unwrap();
        let tag = &html[html[..next].rfind('<').unwrap()..next];
        assert!(tag.contains("data-sourcepos=\"5:1-"), "{tag}");
    }

    #[test]
    fn a_wrap_that_breaks_the_document_is_dropped_and_the_rest_still_shows() {
        let html = html_of(&format!("{DRAWING}#let two() = 2\n#let need(n) = [value #n]\n#need(two())\n\n#drawing()\n"));
        assert!(html.contains("value 2"), "{html}");
        assert!(html.contains("<svg"), "{html}");
    }

    #[test]
    fn a_drawing_in_an_included_file_is_shown_and_named() {
        let dir = temp_project("frame-include");
        fs::write(dir.join("part.typ"), format!("{DRAWING}\n#drawing()\n")).unwrap();
        let main = dir.join("main.typ");
        fs::write(&main, "= Main\n\n#include \"part.typ\"\n").unwrap();
        let Output::Html(html) = TypstRenderer.render(&main).unwrap();
        assert!(html.contains("<svg"), "{html}");
        assert!(html.contains("data-source-file=\"part.typ\""), "{html}");
    }

    #[test]
    fn an_error_still_points_at_the_users_line_after_the_prelude() {
        let dir = temp_project("prelude-lines");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "= A\n\n#undefined-fn()\n");
        let error = rendered.diagnostics.iter().find(|d| d.severity == DiagSeverity::Error).expect("an error");
        assert_eq!((error.file.as_deref(), error.line), (None, 3));
        let message = rendered.output.err().unwrap_or_default();
        assert!(message.contains("error:") && message.contains(":3:"), "{message}");
    }

    #[test]
    fn harmless_drops_like_page_breaks_are_not_shown_as_warnings() {
        let dir = temp_project("harmless");
        let rendered = TypstRenderer.render_buffer(&dir.join("main.typ"), "#set page(width: 10cm)\nA\n#pagebreak()\nB #v(1em)\n");
        assert!(rendered.diagnostics.iter().all(|d| !d.message.contains("was ignored")), "{:?}", rendered.diagnostics);
    }
}
