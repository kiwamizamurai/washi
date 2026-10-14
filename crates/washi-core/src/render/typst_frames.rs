use std::{collections::HashMap, ops::Range};

use typst::syntax::{ast, parse, LinkedNode, SyntaxKind};

pub const PRELUDE: &str = r#"#show align: it => {
  let x = it.alignment.x
  let side = if x == center { "center" } else if x == right or x == end { "right" } else if x == left or x == start { "left" } else { none }
  if side == none { it.body } else { html.elem("div", attrs: (style: "text-align: " + side), it.body) }
}
#show columns: it => html.elem("div", attrs: (style: "column-count: " + str(it.count) + "; column-gap: 2em"), it.body)
#show grid: it => {
  let track(c) = if c == auto { "auto" } else if type(c) == fraction { "minmax(0, " + repr(c) + ")" } else if type(c) in (length, ratio, relative) and not repr(c).contains("+") { repr(c) } else { "auto" }
  let tracks = if type(it.columns) == int { range(calc.max(it.columns, 1)).map(_ => "auto") } else if type(it.columns) == array and it.columns.len() > 0 { it.columns.map(track) } else { ("auto",) }
  let n = tracks.len()
  let x-css(a) = if a == center { "justify-self: center; text-align: center" } else if a == right or a == end { "justify-self: end; text-align: right" } else if a == left or a == start { "justify-self: start; text-align: left" } else { "" }
  let y-css(a) = if a == horizon { "align-self: center" } else if a == bottom { "align-self: end" } else if a == top { "align-self: start" } else { "" }
  let cell-css(column) = {
    let a = if type(it.align) == array and it.align.len() > 0 { it.align.at(calc.rem(column, it.align.len())) } else { it.align }
    if type(a) != alignment { return "" }
    (x-css(a.x), y-css(a.y)).filter(s => s != "").join("; ")
  }
  let column = 0
  let cells = ()
  for c in it.children.filter(c => c.func() != grid.hline and c.func() != grid.vline) {
    let span = if c.func() == grid.cell and c.has("colspan") { c.colspan } else { 1 }
    let body = if c.func() == grid.cell { c.body } else { c }
    cells.push(html.elem("div", attrs: (style: ("grid-column: span " + str(span), cell-css(column)).filter(s => s != "").join("; ")), body))
    column = calc.rem(column + span, n)
  }
  html.elem("div", attrs: (class: "washi-grid", style: "display: grid; grid-template-columns: " + tracks.join(" ") + "; gap: 0.5em 1em"), cells.join())
}
#show stack: it => html.elem("div", attrs: (style: "display: flex; flex-direction: " + (if it.dir == ltr or it.dir == rtl { "row" } else { "column" }) + "; gap: 0.5em"), it.children.filter(c => type(c) == content).map(c => html.elem("div", c)).join())
#show pad: it => it.body
"#;

const BLOCK_OPEN: &str = "html.elem(\"div\", attrs: (class: \"washi-figure\"), html.frame(";
const INLINE_OPEN: &str = "html.elem(\"span\", attrs: (class: \"washi-figure washi-inline\"), html.frame(";
const CLOSE: &str = "))";

const HARMLESS_DROPS: [&str; 7] = ["v", "h", "pagebreak", "colbreak", "linebreak", "parbreak", "page set rule"];

const NON_CONTENT_CALLEES: [&str; 58] = [
    "v", "h", "pagebreak", "colbreak", "linebreak", "parbreak", "counter", "state", "metadata", "update", "step",
    "len", "str", "int", "float", "bool", "rgb", "rgba", "luma", "cmyk", "oklab", "oklch", "color", "range", "array",
    "dictionary", "type", "repr", "query", "here", "locate", "measure", "datetime", "numbering", "regex", "read",
    "json", "yaml", "toml", "csv", "xml", "cbor", "plugin", "label", "selector", "map", "filter", "join", "at",
    "slice", "split", "first", "last", "with", "get", "final", "sum", "zip",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrapKind {
    Frame { block: bool },
    Style(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrap {
    pub range: Range<usize>,
    pub kind: WrapKind,
}

impl Wrap {
    fn opening(&self) -> String {
        match &self.kind {
            WrapKind::Frame { block: true } => BLOCK_OPEN.to_owned(),
            WrapKind::Frame { block: false } => INLINE_OPEN.to_owned(),
            WrapKind::Style(css) => format!("html.elem(\"span\", attrs: (style: \"{css}\"), "),
        }
    }

    fn closing(&self) -> &'static str {
        match self.kind {
            WrapKind::Frame { .. } => CLOSE,
            WrapKind::Style(_) => ")",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edits {
    inserts: Vec<Insert>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Insert {
    at: usize,
    len: usize,
    wraps: Option<Range<usize>>,
}

impl Edits {
    pub fn to_original(&self, offset: usize) -> Option<usize> {
        let mut shift = 0;
        for insert in &self.inserts {
            if offset < insert.at {
                break;
            }
            if offset < insert.at + insert.len {
                return None;
            }
            shift += insert.len;
        }
        Some(offset - shift)
    }

    pub fn wrapped_call_at(&self, offset: usize) -> Option<Range<usize>> {
        self.inserts
            .iter()
            .find(|i| offset >= i.at && offset < i.at + i.len)
            .and_then(|i| i.wraps.clone())
    }
}

pub struct Rewritten {
    pub text: String,
    pub edits: Edits,
}

pub fn rewrite(original: &str, prelude: Option<&str>, wraps: &[Wrap]) -> Rewritten {
    let mut text = String::with_capacity(original.len() + wraps.len() * 96 + prelude.map_or(0, str::len));
    let mut inserts = Vec::new();
    if let Some(prelude) = prelude {
        inserts.push(Insert { at: 0, len: prelude.len(), wraps: None });
        text.push_str(prelude);
    }
    struct Mark {
        at: usize,
        closing: bool,
        other_end: usize,
        piece: String,
        wraps: Option<Range<usize>>,
    }
    let mut marks: Vec<Mark> = Vec::new();
    for wrap in wraps {
        marks.push(Mark { at: wrap.range.start, closing: false, other_end: wrap.range.end, piece: wrap.opening(), wraps: Some(wrap.range.clone()) });
        marks.push(Mark { at: wrap.range.end, closing: true, other_end: wrap.range.start, piece: wrap.closing().to_owned(), wraps: None });
    }
    marks.sort_by(|a, b| {
        a.at.cmp(&b.at)
            .then(b.closing.cmp(&a.closing))
            .then(if a.closing { b.other_end.cmp(&a.other_end) } else { b.other_end.cmp(&a.other_end) })
    });
    let mut copied = 0;
    for mark in marks {
        text.push_str(&original[copied..mark.at]);
        copied = mark.at;
        inserts.push(Insert { at: text.len(), len: mark.piece.len(), wraps: mark.wraps });
        text.push_str(&mark.piece);
    }
    text.push_str(&original[copied..]);
    Rewritten { text, edits: Edits { inserts } }
}

pub fn is_significant_drop(message: &str) -> bool {
    let Some(name) = message.strip_suffix(" was ignored during HTML export") else {
        return false;
    };
    !HARMLESS_DROPS.contains(&name)
}

pub fn is_harmless_drop(message: &str) -> bool {
    message
        .strip_suffix(" was ignored during HTML export")
        .is_some_and(|name| HARMLESS_DROPS.contains(&name) || name.ends_with("set rule"))
}

fn has_output_inside(covered: &[Range<usize>], range: &Range<usize>) -> bool {
    covered.iter().any(|c| c.start >= range.start && c.end <= range.end && c.start < c.end)
}

fn contains_any(drops: &[Range<usize>], range: &Range<usize>) -> bool {
    drops.iter().any(|d| d.start >= range.start && d.end <= range.end)
}

#[derive(Debug, Clone, Default)]
pub struct Evidence {
    pub covered: Vec<Range<usize>>,
    pub drops_here: Vec<Range<usize>>,
    pub drops_elsewhere: bool,
}

enum Callee<'a> {
    Local(&'a str),
    Builtin,
    External,
}

fn callee_name<'a>(call: ast::FuncCall<'a>) -> Option<&'a str> {
    match call.callee() {
        ast::Expr::Ident(ident) => Some(ident.as_str()),
        ast::Expr::FieldAccess(access) => Some(access.field().as_str()),
        _ => None,
    }
}

fn classify<'a>(call: ast::FuncCall<'a>, locals: &HashMap<String, Range<usize>>, imported: &[String], is_builtin: &dyn Fn(&str) -> bool) -> Callee<'a> {
    match call.callee() {
        ast::Expr::Ident(ident) => {
            let name = ident.as_str();
            if imported.iter().any(|n| n == name) {
                Callee::External
            } else if locals.contains_key(name) {
                Callee::Local(name)
            } else if is_builtin(name) {
                Callee::Builtin
            } else {
                Callee::External
            }
        }
        _ => Callee::External,
    }
}

fn local_functions(root: &LinkedNode) -> HashMap<String, Range<usize>> {
    let mut found = HashMap::new();
    collect_locals(root, &mut found);
    found
}

fn collect_locals(node: &LinkedNode, found: &mut HashMap<String, Range<usize>>) {
    if let Some(binding) = node.cast::<ast::LetBinding>() {
        let name = match binding.kind() {
            ast::LetBindingKind::Closure(ident) => Some(ident.as_str().to_owned()),
            ast::LetBindingKind::Normal(ast::Pattern::Normal(ast::Expr::Ident(ident))) => Some(ident.as_str().to_owned()),
            _ => None,
        };
        if let Some(name) = name {
            found.insert(name, node.range());
        }
    }
    for child in node.children() {
        collect_locals(&child, found);
    }
}

fn imported_names(root: &LinkedNode) -> Vec<String> {
    let mut names = Vec::new();
    collect_imports(root, &mut names);
    names
}

fn collect_imports(node: &LinkedNode, names: &mut Vec<String>) {
    if let Some(import) = node.cast::<ast::ModuleImport>() {
        if let Some(ast::Imports::Items(items)) = import.imports() {
            for item in items.iter() {
                names.push(item.bound_name().as_str().to_owned());
            }
        }
        if let Some(alias) = import.new_name() {
            names.push(alias.as_str().to_owned());
        }
        return;
    }
    for child in node.children() {
        collect_imports(&child, names);
    }
}

fn produces_content_here(node: &LinkedNode) -> bool {
    matches!(
        node.parent().map(|parent| parent.kind()),
        Some(SyntaxKind::Markup | SyntaxKind::Args | SyntaxKind::Code)
    )
}

fn stands_alone(text: &str, range: &Range<usize>) -> bool {
    let line_start = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[range.end..].find('\n').map_or(text.len(), |i| range.end + i);
    let before = text[line_start..range.start].trim();
    let after = text[range.end..line_end].trim();
    (before.is_empty() || before == "#") && after.is_empty()
}

pub fn find_dropped_calls(text: &str, evidence: &Evidence, is_builtin: &dyn Fn(&str) -> bool) -> Vec<Wrap> {
    let root = parse(text);
    let linked = LinkedNode::new(&root);
    let scan = Scan {
        text,
        evidence,
        is_builtin,
        locals: local_functions(&linked),
        imported: imported_names(&linked),
    };
    let mut found = Vec::new();
    scan.visit(&linked, &mut found);
    found
}

struct Scan<'a> {
    text: &'a str,
    evidence: &'a Evidence,
    is_builtin: &'a dyn Fn(&str) -> bool,
    locals: HashMap<String, Range<usize>>,
    imported: Vec<String>,
}

impl Scan<'_> {
    fn needs_frame(&self, call: ast::FuncCall, range: &Range<usize>) -> bool {
        match classify(call, &self.locals, &self.imported, self.is_builtin) {
            Callee::Local(name) => self.locals.get(name).is_some_and(|body| contains_any(&self.evidence.drops_here, body)),
            Callee::Builtin => contains_any(&self.evidence.drops_here, range),
            Callee::External => self.evidence.drops_elsewhere,
        }
    }

    fn visit(&self, node: &LinkedNode, found: &mut Vec<Wrap>) {
        match node.kind() {
            SyntaxKind::LetBinding
            | SyntaxKind::SetRule
            | SyntaxKind::ShowRule
            | SyntaxKind::ModuleImport
            | SyntaxKind::ModuleInclude
            | SyntaxKind::Closure
            | SyntaxKind::Named => return,
            SyntaxKind::FuncCall => {
                if let Some(call) = node.cast::<ast::FuncCall>() {
                    let range = node.range();
                    let name = callee_name(call).unwrap_or("");
                    let candidate = produces_content_here(node) && !NON_CONTENT_CALLEES.contains(&name);
                    if candidate && !has_output_inside(&self.evidence.covered, &range) && self.needs_frame(call, &range) {
                        let block = node.parent().map(|parent| parent.kind()) != Some(SyntaxKind::Markup) || stands_alone(self.text, &range);
                        found.push(Wrap { range, kind: WrapKind::Frame { block } });
                        return;
                    }
                }
            }
            _ => {}
        }
        for child in node.children() {
            self.visit(&child, found);
        }
    }
}

const NAMED_COLORS: [(&str, &str); 18] = [
    ("black", "#000000"), ("gray", "#aaaaaa"), ("silver", "#dddddd"), ("white", "#ffffff"),
    ("navy", "#001f3f"), ("blue", "#0074d9"), ("aqua", "#7fdbff"), ("teal", "#39cccc"),
    ("eastern", "#239dad"), ("purple", "#b10dc9"), ("fuchsia", "#f012be"), ("maroon", "#85144b"),
    ("red", "#ff4136"), ("orange", "#ff851b"), ("yellow", "#ffdc00"), ("olive", "#3d9970"),
    ("green", "#2ecc40"), ("lime", "#01ff70"),
];

const WEIGHTS: [(&str, u16); 9] = [
    ("thin", 100), ("extralight", 200), ("light", 300), ("regular", 400), ("medium", 500),
    ("semibold", 600), ("bold", 700), ("extrabold", 800), ("black", 900),
];

const DARK_TEXT_LIMIT: f64 = 0.35;

fn length_css(expr: ast::Expr) -> Option<String> {
    let ast::Expr::Numeric(numeric) = expr else { return None };
    let (value, unit) = numeric.get();
    let unit = match unit {
        typst::syntax::ast::Unit::Pt => "pt",
        typst::syntax::ast::Unit::Mm => "mm",
        typst::syntax::ast::Unit::Cm => "cm",
        typst::syntax::ast::Unit::In => "in",
        typst::syntax::ast::Unit::Em => "em",
        _ => return None,
    };
    (value > 0.0).then(|| format!("{value}{unit}"))
}

fn weight_css(expr: ast::Expr) -> Option<u16> {
    match expr {
        ast::Expr::Str(s) => WEIGHTS.iter().find(|(name, _)| *name == s.get().as_str()).map(|(_, w)| *w),
        ast::Expr::Int(i) => u16::try_from(i.get()).ok().filter(|w| (100..=900).contains(w)),
        _ => None,
    }
}

fn hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    let full = match hex.len() {
        3 | 4 => hex.chars().take(3).flat_map(|c| [c, c]).collect::<String>(),
        6 | 8 => hex[..6].to_owned(),
        _ => return None,
    };
    let channel = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

fn color_css(expr: ast::Expr) -> Option<String> {
    let (r, g, b) = match expr {
        ast::Expr::Ident(ident) => hex_rgb(NAMED_COLORS.iter().find(|(name, _)| *name == ident.as_str())?.1)?,
        ast::Expr::FuncCall(call) => {
            let ast::Expr::Ident(func) = call.callee() else { return None };
            let first = call.args().items().find_map(|arg| match arg {
                ast::Arg::Pos(expr) => Some(expr),
                _ => None,
            })?;
            match (func.as_str(), first) {
                ("rgb", ast::Expr::Str(hex)) => hex_rgb(hex.get().as_str())?,
                ("luma", ast::Expr::Int(n)) => {
                    let n = u8::try_from(n.get()).ok()?;
                    (n, n, n)
                }
                ("luma", ast::Expr::Numeric(numeric)) => {
                    let (value, unit) = numeric.get();
                    if !matches!(unit, typst::syntax::ast::Unit::Percent) {
                        return None;
                    }
                    let n = (value / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
                    (n, n, n)
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let luminance = (0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)) / 255.0;
    (luminance >= DARK_TEXT_LIMIT).then(|| format!("#{r:02x}{g:02x}{b:02x}"))
}

fn text_style(call: ast::FuncCall) -> Option<String> {
    let mut has_body = false;
    let mut css = Vec::new();
    for arg in call.args().items() {
        match arg {
            ast::Arg::Pos(ast::Expr::ContentBlock(_)) => has_body = true,
            ast::Arg::Pos(ast::Expr::Numeric(_)) => {
                if let ast::Arg::Pos(expr) = arg {
                    if let Some(size) = length_css(expr) {
                        css.push(format!("font-size: {size}"));
                    }
                }
            }
            ast::Arg::Named(named) => match named.name().as_str() {
                "size" => css.extend(length_css(named.expr()).map(|v| format!("font-size: {v}"))),
                "weight" => css.extend(weight_css(named.expr()).map(|v| format!("font-weight: {v}"))),
                "style" => {
                    if let ast::Expr::Str(style) = named.expr() {
                        if matches!(style.get().as_str(), "italic" | "oblique") {
                            css.push(format!("font-style: {}", style.get()));
                        }
                    }
                }
                "fill" => css.extend(color_css(named.expr()).map(|v| format!("color: {v}"))),
                _ => {}
            },
            _ => {}
        }
    }
    (has_body && !css.is_empty()).then(|| css.join("; "))
}

pub fn styled_text_wraps(text: &str) -> Vec<Wrap> {
    let root = parse(text);
    let mut found = Vec::new();
    collect_styled(&LinkedNode::new(&root), &mut found);
    found
}

fn collect_styled(node: &LinkedNode, found: &mut Vec<Wrap>) {
    match node.kind() {
        SyntaxKind::LetBinding
        | SyntaxKind::SetRule
        | SyntaxKind::ShowRule
        | SyntaxKind::ModuleImport
        | SyntaxKind::Closure
        | SyntaxKind::Named => return,
        SyntaxKind::FuncCall => {
            if let Some(call) = node.cast::<ast::FuncCall>() {
                let is_text = matches!(call.callee(), ast::Expr::Ident(ident) if ident.as_str() == "text");
                if is_text && produces_content_here(node) {
                    if let Some(css) = text_style(call) {
                        found.push(Wrap { range: node.range(), kind: WrapKind::Style(css) });
                    }
                }
            }
        }
        _ => {}
    }
    for child in node.children() {
        collect_styled(&child, found);
    }
}

pub fn include_paths(text: &str) -> Vec<String> {
    let root = parse(text);
    let mut paths = Vec::new();
    collect_includes(&LinkedNode::new(&root), &mut paths);
    paths
}

fn collect_includes(node: &LinkedNode, paths: &mut Vec<String>) {
    if let Some(include) = node.cast::<ast::ModuleInclude>() {
        if let ast::Expr::Str(path) = include.source() {
            paths.push(path.get().to_string());
        }
        return;
    }
    for child in node.children() {
        collect_includes(&child, paths);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILTINS: [&str; 12] = ["figure", "align", "place", "text", "str", "rect", "box", "image", "v", "counter", "lorem", "rotate"];

    fn builtin(name: &str) -> bool {
        BUILTINS.contains(&name)
    }

    fn range_of(text: &str, needle: &str) -> Range<usize> {
        let start = text.find(needle).expect("needle in text");
        start..start + needle.len()
    }

    fn wraps_with(text: &str, evidence: &Evidence) -> Vec<String> {
        find_dropped_calls(text, evidence, &builtin).into_iter().map(|w| text[w.range].to_owned()).collect()
    }

    fn external(covered: Vec<Range<usize>>) -> Evidence {
        Evidence { covered, drops_here: Vec::new(), drops_elsewhere: true }
    }

    #[test]
    fn a_package_call_with_no_output_is_found_when_a_package_dropped_something() {
        let text = "#import \"@preview/cetz:0.3.4\": canvas\n= Title\n\n#canvas({ line((0, 0), (1, 1)) })\n\nText after.\n";
        let evidence = external(vec![range_of(text, "Title"), range_of(text, "Text after.")]);
        assert_eq!(wraps_with(text, &evidence), ["canvas({ line((0, 0), (1, 1)) })"]);
    }

    #[test]
    fn nothing_is_wrapped_when_nothing_outside_was_dropped() {
        let text = "#import \"@preview/x:0.1.0\": thing\n#thing()\n";
        assert!(wraps_with(text, &Evidence::default()).is_empty());
    }

    #[test]
    fn a_call_that_produced_output_is_left_alone() {
        let text = "#figure(image(\"a.png\"), caption: [A])\n";
        let evidence = external(vec![range_of(text, "image(\"a.png\")"), range_of(text, "A")]);
        assert!(wraps_with(text, &evidence).is_empty());
    }

    #[test]
    fn inside_a_figure_only_the_dropped_body_is_wrapped() {
        let text = "#figure(drawing(), caption: [Shown])\n";
        let whole = 1..text.trim_end().len();
        let evidence = external(vec![whole, range_of(text, "Shown")]);
        assert_eq!(wraps_with(text, &evidence), ["drawing()"]);
    }

    #[test]
    fn a_local_function_is_wrapped_only_when_its_own_body_dropped_something() {
        let text = "#let pic() = context layout(_ => none)\n#let today() = [Oct 6]\n#pic()\n#today()\n";
        let body = range_of(text, "layout(_ => none)");
        let evidence = Evidence { covered: vec![range_of(text, "Oct 6")], drops_here: vec![body], drops_elsewhere: false };
        assert_eq!(wraps_with(text, &evidence), ["pic()"]);
    }

    #[test]
    fn a_builtin_is_wrapped_only_when_the_drop_is_inside_the_call() {
        let text = "#place(top)[Moved]\n#rotate(10deg)[Turned]\n";
        let evidence = Evidence { covered: Vec::new(), drops_here: vec![range_of(text, "place(top)[Moved]")], drops_elsewhere: true };
        assert_eq!(wraps_with(text, &evidence), ["place(top)[Moved]"]);
    }

    #[test]
    fn named_arguments_and_non_content_calls_are_never_wrapped() {
        let text = "#text(fill: rgb(\"#f00\"))[red] #str(len(\"ab\"))\n#v(1em)\n#counter(page).update(1)\n";
        let evidence = external(vec![range_of(text, "red"), range_of(text, "str(len(\"ab\"))")]);
        assert!(wraps_with(text, &evidence).is_empty(), "{:?}", wraps_with(text, &evidence));
    }

    #[test]
    fn definitions_imports_and_rules_are_skipped() {
        let text = "#import \"@preview/cetz:0.3.4\": canvas\n#let pic = canvas({})\n#show heading: it => drawing(it)\n#set text(size: 10pt)\n";
        assert!(wraps_with(text, &external(Vec::new())).is_empty());
    }

    #[test]
    fn module_members_count_as_external() {
        let text = "#import \"@preview/fletcher:0.5.8\" as fletcher\n#fletcher.diagram(node((0, 0), [A]))\n";
        assert_eq!(wraps_with(text, &external(Vec::new())), ["fletcher.diagram(node((0, 0), [A]))"]);
    }

    #[test]
    fn a_call_alone_on_its_line_is_a_block_and_one_in_a_sentence_is_inline() {
        let text = "#drawing()\n\nsee #icon() here\n";
        let found = find_dropped_calls(text, &external(vec![range_of(text, "see"), range_of(text, "here")]), &builtin);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, WrapKind::Frame { block: true });
        assert_eq!(found[1].kind, WrapKind::Frame { block: false });
    }

    #[test]
    fn rewriting_wraps_calls_and_maps_offsets_back() {
        let original = "A #drawing() B";
        let call = range_of(original, "drawing()");
        let out = rewrite(original, Some("#prelude\n"), &[Wrap { range: call.clone(), kind: WrapKind::Frame { block: false } }]);
        assert!(out.text.starts_with("#prelude\nA #html.elem(\"span\""), "{}", out.text);
        assert!(out.text.contains("html.frame(drawing())) B"), "{}", out.text);
        let b = out.text.rfind('B').unwrap();
        assert_eq!(out.edits.to_original(b), Some(original.rfind('B').unwrap()));
        let a = out.text.find("A ").unwrap();
        assert_eq!(out.edits.to_original(a), Some(0));
        assert_eq!(out.edits.to_original(1), None, "inside the prelude");
        let inside_call = out.text.find("drawing()").unwrap();
        assert_eq!(out.edits.to_original(inside_call), Some(call.start));
        let opening = out.text.find("html.elem").unwrap();
        assert_eq!(out.edits.to_original(opening), None);
        assert_eq!(out.edits.wrapped_call_at(opening), Some(call));
    }

    #[test]
    fn adjacent_wraps_close_before_the_next_opens() {
        let original = "#a()#b()";
        let wraps = [
            Wrap { range: range_of(original, "a()"), kind: WrapKind::Frame { block: false } },
            Wrap { range: range_of(original, "b()"), kind: WrapKind::Frame { block: false } },
        ];
        let out = rewrite(original, None, &wraps);
        assert!(out.text.contains("html.frame(a()))#html.elem"), "{}", out.text);
    }

    #[test]
    fn only_layout_drops_count_as_significant() {
        assert!(is_significant_drop("layout was ignored during HTML export"));
        assert!(is_significant_drop("place was ignored during HTML export"));
        assert!(!is_significant_drop("v was ignored during HTML export"));
        assert!(!is_significant_drop("pagebreak was ignored during HTML export"));
        assert!(!is_significant_drop("unknown font family: x"));
        assert!(is_harmless_drop("page set rule was ignored during HTML export"));
        assert!(is_harmless_drop("colbreak was ignored during HTML export"));
        assert!(!is_harmless_drop("layout was ignored during HTML export"));
    }

    #[test]
    fn included_files_are_listed() {
        assert_eq!(include_paths("= A\n#include \"part.typ\"\n#include \"ch/two.typ\"\n"), ["part.typ", "ch/two.typ"]);
    }

    fn styles(text: &str) -> Vec<(String, String)> {
        styled_text_wraps(text)
            .into_iter()
            .map(|w| match w.kind {
                WrapKind::Style(css) => (text[w.range].to_owned(), css),
                WrapKind::Frame { .. } => unreachable!(),
            })
            .collect()
    }

    #[test]
    fn a_sized_bold_title_becomes_a_styled_span() {
        let text = "#align(center)[\n  #text(size: 24pt, weight: \"bold\")[Washi Technical Report]\n]\n";
        assert_eq!(styles(text), [("text(size: 24pt, weight: \"bold\")[Washi Technical Report]".to_owned(), "font-size: 24pt; font-weight: 700".to_owned())]);
    }

    #[test]
    fn literal_colours_are_kept_but_dark_ones_are_left_to_the_theme() {
        let text = "#text(fill: luma(100))[a] #text(fill: luma(200))[b] #text(fill: rgb(\"#b5483a\"), style: \"italic\")[c] #text(fill: black)[d] #text(fill: red)[e]";
        let found = styles(text);
        let css: Vec<&str> = found.iter().map(|(_, css)| css.as_str()).collect();
        assert_eq!(css, ["color: #646464", "color: #c8c8c8", "color: #b5483a; font-style: italic", "color: #ff4136"], "black is left to the theme: {found:?}");
    }

    #[test]
    fn a_positional_size_counts_too() {
        assert_eq!(styles("#text(14pt)[x]")[0].1, "font-size: 14pt");
    }

    #[test]
    fn calls_without_a_body_or_with_computed_values_are_left_alone() {
        assert!(styles("#text(size: 9pt) #text(size: base * 2)[x] #text(weight: w)[y] #set text(size: 9pt)").is_empty());
    }

    #[test]
    fn a_style_inside_a_frame_nests_inside_it() {
        let original = "#box(text(size: 2em)[A])";
        let outer = range_of(original, "box(text(size: 2em)[A])");
        let inner = range_of(original, "text(size: 2em)[A]");
        let wraps = [
            Wrap { range: inner, kind: WrapKind::Style("font-size: 2em".into()) },
            Wrap { range: outer, kind: WrapKind::Frame { block: true } },
        ];
        let out = rewrite(original, None, &wraps);
        let frame = out.text.find("html.frame(").unwrap();
        let span = out.text.find("html.elem(\"span\"").unwrap();
        assert!(frame < span, "{}", out.text);
        assert!(out.text.ends_with("[A]))))"), "span, box, then the frame close in that order: {}", out.text);
    }
}
