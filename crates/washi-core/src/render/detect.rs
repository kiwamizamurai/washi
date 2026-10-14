const MERMAID_STARTS: [&str; 14] = [
    "graph",
    "flowchart",
    "sequenceDiagram",
    "classDiagram",
    "stateDiagram",
    "erDiagram",
    "gantt",
    "pie",
    "journey",
    "mindmap",
    "timeline",
    "gitGraph",
    "quadrantChart",
    "xychart",
];

const TYPST_STARTS: [&str; 6] = ["#set ", "#let ", "#import ", "#show ", "#include ", "#import("];

pub fn extension_of_text(text: &str) -> &'static str {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");

    if is_tex(text) {
        "tex"
    } else if is_mermaid(first) {
        "mmd"
    } else if is_typst(text, first) {
        "typ"
    } else {
        "md"
    }
}

fn is_tex(text: &str) -> bool {
    text.contains("\\documentclass") || text.contains("\\begin{document}")
}

fn is_mermaid(first: &str) -> bool {
    MERMAID_STARTS
        .iter()
        .any(|s| first == *s || first.starts_with(&format!("{s} ")) || first.starts_with(&format!("{s}-")))
}

fn is_typst(text: &str, first: &str) -> bool {
    if TYPST_STARTS.iter().any(|s| first.starts_with(s)) {
        return true;
    }
    let is_heading = |prefix: char| {
        text.lines().any(|l| {
            let rest = l.trim_start_matches(prefix);
            l.starts_with(prefix) && l.len() > rest.len() && rest.starts_with(' ') && !rest.trim().is_empty()
        })
    };
    is_heading('=') && !is_heading('#')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_latex_documents() {
        assert_eq!(extension_of_text("\\documentclass{article}\n\\begin{document}x\\end{document}"), "tex");
        assert_eq!(extension_of_text("\\begin{document}\nHi\n\\end{document}"), "tex");
    }

    #[test]
    fn detects_mermaid_diagrams() {
        for src in ["graph LR; A-->B", "flowchart TD\n  A-->B", "sequenceDiagram\n A->>B: hi", "\n\n  pie title x\n"] {
            assert_eq!(extension_of_text(src), "mmd", "{src}");
        }
        assert_eq!(extension_of_text("graphite is a mineral"), "md");
    }

    #[test]
    fn detects_typst_by_directives_or_equals_headings() {
        assert_eq!(extension_of_text("#set page(paper: \"a4\")\n= Title"), "typ");
        assert_eq!(extension_of_text("#import \"@preview/x:0.1.0\": *"), "typ");
        assert_eq!(extension_of_text("= Title\nSome *bold* text.\n== Sub"), "typ");
    }

    #[test]
    fn markdown_is_the_default() {
        assert_eq!(extension_of_text("# Title\n\ntext"), "md");
        assert_eq!(extension_of_text("plain text"), "md");
        assert_eq!(extension_of_text(""), "md");
        assert_eq!(extension_of_text("# H\n= not typst\n"), "md");
        assert_eq!(extension_of_text("a = b\n"), "md");
    }
}
