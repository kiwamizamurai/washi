use std::sync::LazyLock;

use regex::Regex;

const UNSAFE_ENVIRONMENTS: [&str; 22] = [
    "equation", "align", "alignat", "flalign", "gather", "multline", "eqnarray", "displaymath", "math", "split", "array",
    "tabular", "tabularx", "longtable", "verbatim", "lstlisting", "minted", "tikzpicture", "filecontents", "comment",
    "thebibliography", "matrix",
];

const SECTIONING: [&str; 8] = [
    "\\part", "\\chapter", "\\section", "\\subsection", "\\subsubsection", "\\paragraph", "\\subparagraph", "\\item",
];

static ITEM_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\\item(\[[^\]]*\])?\s*").unwrap());
static ENVIRONMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\(begin|end)\{([A-Za-z]+)\*?\}").unwrap());
const MARKER_HTML: &str = r#"<(?:div|span) id="washi-L(\d+)">\s*</(?:div|span)>\s*"#;

static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(MARKER_HTML).unwrap());
static MARKERS_BEFORE_ELEMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("(?:{MARKER_HTML})+<([A-Za-z][A-Za-z0-9]*)")).unwrap());

fn marker(line: usize) -> String {
    format!("\\hypertarget{{washi-L{line}}}{{}}")
}

fn is_unsafe(name: &str) -> bool {
    UNSAFE_ENVIRONMENTS.contains(&name)
}

fn starts_unsafe_environment(line: &str) -> bool {
    ENVIRONMENT
        .captures_iter(line)
        .any(|c| &c[1] == "begin" && is_unsafe(&c[2]))
}

fn is_boundary(line: &str) -> bool {
    line.starts_with("\\begin{")
        || line.starts_with("\\end{")
        || SECTIONING.iter().any(|s| *s != "\\item" && line.starts_with(s))
}

fn display_math_toggles(line: &str) -> usize {
    line.matches("\\[").count() + line.matches("\\]").count() + line.matches("$$").count()
}

pub fn inject(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + source.len() / 10);
    let mut in_body = false;
    let mut unsafe_depth: usize = 0;
    let mut in_display = false;
    let mut previous_blank = true;
    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        let blank = trimmed.is_empty();
        let comment = trimmed.starts_with('%');
        let begins_paragraph =
            previous_blank || trimmed.starts_with("\\begin{") || SECTIONING.iter().any(|s| trimmed.starts_with(s));
        let markable = in_body
            && !trimmed.starts_with("\\end{document}")
            && unsafe_depth == 0
            && !in_display
            && !blank
            && !comment
            && begins_paragraph
            && !starts_unsafe_environment(trimmed)
            && display_math_toggles(trimmed) == 0;

        if markable {
            if let Some(head) = ITEM_HEAD.find(trimmed) {
                out.push_str(indent);
                out.push_str(head.as_str());
                out.push_str(&marker(number));
                out.push_str(&trimmed[head.end()..]);
            } else {
                out.push_str(indent);
                out.push_str(&marker(number));
                out.push_str(trimmed);
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');

        if trimmed.starts_with("\\begin{document}") {
            in_body = true;
        }
        for c in ENVIRONMENT.captures_iter(trimmed) {
            if is_unsafe(&c[2]) {
                if &c[1] == "begin" {
                    unsafe_depth += 1;
                } else {
                    unsafe_depth = unsafe_depth.saturating_sub(1);
                }
            }
        }
        if display_math_toggles(trimmed) % 2 == 1 {
            in_display = !in_display;
        }
        if !comment {
            previous_blank = blank || is_boundary(trimmed);
        }
    }
    if !source.ends_with('\n') {
        out.pop();
    }
    out
}

pub fn read_back(html: &str) -> String {
    let stamped = MARKERS_BEFORE_ELEMENT.replace_all(html, |c: &regex::Captures| {
        let nearest = MARKER.captures_iter(&c[0]).last().map_or("1", |m| m.get(1).map_or("1", |n| n.as_str()));
        format!("<{} data-sourcepos=\"{nearest}:1-{nearest}:1\"", &c[2])
    });
    MARKER.replace_all(&stamped, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = "\\documentclass{article}\n\\begin{document}\n\\section{Intro}\nFirst paragraph.\n\nSecond paragraph\ncontinues here.\n\\begin{itemize}\n\\item One\n\\item[x] Two\n\\end{itemize}\n\\end{document}\n";

    fn marked(source: &str) -> Vec<usize> {
        inject(source)
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("\\hypertarget{washi-L"))
            .map(|(i, _)| i + 1)
            .collect()
    }

    #[test]
    fn marks_the_start_of_each_paragraph_and_list_item_without_moving_any_line() {
        let out = inject(DOCUMENT);
        assert_eq!(out.lines().count(), DOCUMENT.lines().count());
        assert_eq!(marked(DOCUMENT), [3, 4, 6, 8, 9, 10], "{out}");
        assert!(out.contains("\\hypertarget{washi-L3}{}\\section{Intro}"), "{out}");
        assert!(out.contains("\\item \\hypertarget{washi-L9}{}One") || out.contains("\\item\\hypertarget{washi-L9}{}One"), "{out}");
        assert!(out.contains("\\item[x] \\hypertarget{washi-L10}{}Two"), "{out}");
    }

    #[test]
    fn leaves_the_preamble_alone() {
        let out = inject("\\documentclass{article}\n\n\\usepackage{x}\n\n\\newcommand{\\a}{b}\n\\begin{document}\nText.\n\n\\end{document}\n");
        assert!(!out.lines().take(6).any(|l| l.contains("hypertarget")), "{out}");
    }

    #[test]
    fn keeps_out_of_math_tables_verbatim_and_figures_drawn_with_tikz() {
        let source = "\\begin{document}\nText.\n\n\\begin{align}\na &= b \\\\\n\nc &= d\n\\end{align}\n\n\\begin{tabular}{ll}\n\nx & y\n\\end{tabular}\n\n\\begin{verbatim}\n\nraw text\n\\end{verbatim}\n\nAfter.\n\\end{document}\n";
        assert_eq!(marked(source), [2, 20], "{}", inject(source));
    }

    #[test]
    fn keeps_out_of_display_math_written_with_brackets_or_dollars() {
        let source = "\\begin{document}\nText.\n\n\\[\n\nx = 1\n\\]\n\nMore.\n\n$$\n\ny\n$$\n\nEnd.\n";
        assert_eq!(marked(source), [2, 9, 16], "{}", inject(source));
    }

    #[test]
    fn skips_comment_lines_and_does_not_let_them_split_a_paragraph() {
        let source = "\\begin{document}\nStart of a paragraph\n% a comment\ncontinues here.\n\nNext.\n";
        assert_eq!(marked(source), [2, 6], "{}", inject(source));
    }

    #[test]
    fn preserves_a_missing_final_newline() {
        assert!(!inject("\\begin{document}\nText").ends_with('\n'));
        assert!(inject("\\begin{document}\nText\n").ends_with('\n'));
    }

    #[test]
    fn turns_a_marker_before_a_block_into_its_source_position() {
        let html = "<div id=\"washi-L3\">\n\n</div>\n<h1 id=\"intro\">Intro</h1>\n<div id=\"washi-L4\">\n\n</div>\n<p>Text</p>";
        assert_eq!(
            read_back(html),
            "<h1 data-sourcepos=\"3:1-3:1\" id=\"intro\">Intro</h1>\n<p data-sourcepos=\"4:1-4:1\">Text</p>"
        );
    }

    #[test]
    fn a_marker_inside_a_list_item_marks_the_paragraph_that_follows_it() {
        let html = "<ul>\n<li><div id=\"washi-L8\">\n\n</div>\n<p>First</p></li>\n</ul>";
        assert_eq!(read_back(html), "<ul>\n<li><p data-sourcepos=\"8:1-8:1\">First</p></li>\n</ul>");
    }

    #[test]
    fn a_marker_with_nothing_after_it_is_dropped() {
        assert_eq!(read_back("<p>a</p><div id=\"washi-L9\"></div>"), "<p>a</p>");
    }

    #[test]
    fn spans_work_like_divs_and_ordinary_ids_are_untouched() {
        assert_eq!(
            read_back("<span id=\"washi-L2\"></span><p>x</p><div id=\"other\"></div>"),
            "<p data-sourcepos=\"2:1-2:1\">x</p><div id=\"other\"></div>"
        );
    }

    #[test]
    fn several_markers_in_a_row_all_go_and_the_nearest_one_wins() {
        let html = "<div id=\"washi-L18\">\n\n</div>\n<div id=\"washi-L20\">\n\n</div>\n<h1>Title</h1>";
        assert_eq!(read_back(html), "<h1 data-sourcepos=\"20:1-20:1\">Title</h1>");
    }
}
