use std::{collections::HashMap, sync::LazyLock};

use regex::{Captures, Regex};

const NUMBERED_SINGLE: [&str; 2] = ["equation", "multline"];
const NUMBERED_ROWS: [&str; 5] = ["align", "gather", "flalign", "alignat", "eqnarray"];

static DISPLAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\begin\{(equation|multline|align|gather|flalign|alignat|eqnarray|displaymath|math)(\*?)\}|\\\[|\$\$").unwrap()
});
static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\label\{([^}]+)\}").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\tag\*?\{([^}]+)\}").unwrap());
static NO_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\(notag|nonumber)\b").unwrap());
static BLOCK_MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<math display="block""#).unwrap());
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(<a [^>]*data-reference-type="(eqref|ref)"[^>]*data-reference="([^"]+)"[^>]*>)\[[^\]<]*\](</a>)"#).unwrap()
});

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Numbering {
    pub labels: HashMap<String, String>,
    pub blocks: Vec<Vec<String>>,
    first_label: Vec<Option<String>>,
}

fn strip_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let mut escaped = false;
            for (i, c) in line.char_indices() {
                if c == '%' && !escaped {
                    return &line[..i];
                }
                escaped = c == '\\' && !escaped;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_rows(body: &str) -> Vec<&str> {
    let mut rows = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' if depth == 0 && bytes.get(i + 1) == Some(&b'\\') => {
                rows.push(&body[start..i]);
                i += 2;
                start = i;
                continue;
            }
            b'\\' => {
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let last = &body[start..];
    if !last.trim().is_empty() || rows.is_empty() {
        rows.push(last);
    }
    rows
}

pub fn number(source: &str) -> Numbering {
    let text = strip_comments(source);
    let body = text.find("\\begin{document}").map_or(text.as_str(), |at| &text[at..]);
    let mut numbering = Numbering::default();
    let mut counter = 0u32;
    let mut at = 0;
    while let Some(found) = DISPLAY.captures(&body[at..]) {
        let whole = found.get(0).expect("a match has a whole range");
        let start = at + whole.end();
        let (name, starred) = (found.get(1).map(|m| m.as_str()), found.get(2).is_some_and(|m| !m.as_str().is_empty()));
        let close = match (name, whole.as_str()) {
            (Some(name), _) => format!("\\end{{{name}{}}}", if starred { "*" } else { "" }),
            (None, "\\[") => "\\]".to_owned(),
            _ => "$$".to_owned(),
        };
        let Some(end) = body[start..].find(&close).map(|i| start + i) else { break };
        let inner = &body[start..end];
        let numbered = !starred && name.is_some_and(|n| NUMBERED_SINGLE.contains(&n) || NUMBERED_ROWS.contains(&n));
        let rows: Vec<&str> = if name.is_some_and(|n| NUMBERED_ROWS.contains(&n)) { split_rows(inner) } else { vec![inner] };
        let mut shown = Vec::new();
        let mut first_label = None;
        for row in rows {
            let tag = TAG.captures(row).map(|c| c[1].to_owned());
            let number = match tag {
                Some(tag) => Some(tag),
                None if numbered && !NO_NUMBER.is_match(row) => {
                    counter += 1;
                    Some(counter.to_string())
                }
                None => None,
            };
            if let Some(number) = number {
                for label in LABEL.captures_iter(row) {
                    numbering.labels.insert(label[1].to_owned(), number.clone());
                    first_label.get_or_insert_with(|| label[1].to_owned());
                }
                shown.push(number);
            }
        }
        numbering.blocks.push(shown);
        numbering.first_label.push(first_label);
        at = end + close.len();
    }
    numbering
}

pub fn apply(html: &str, numbering: &Numbering) -> String {
    let blocks = BLOCK_MATH.find_iter(html).count();
    let mut out = if blocks == numbering.blocks.len() {
        let mut index = 0;
        BLOCK_MATH
            .replace_all(html, |_: &Captures| {
                let numbers = &numbering.blocks[index];
                let label = numbering.first_label[index].as_deref();
                index += 1;
                let tag = if numbers.is_empty() {
                    String::new()
                } else {
                    let text = numbers.iter().map(|n| format!("({n})")).collect::<Vec<_>>().join(" ");
                    let id = label.map(|l| format!(" id=\"{}\"", escape(l))).unwrap_or_default();
                    format!("<span class=\"eq-number\"{id}>{text}</span>")
                };
                format!("{tag}<math display=\"block\"")
            })
            .into_owned()
    } else {
        html.to_owned()
    };
    out = REFERENCE
        .replace_all(&out, |c: &Captures| {
            let Some(number) = numbering.labels.get(&c[3]) else { return c[0].to_owned() };
            let text = if &c[2] == "eqref" { format!("({number})") } else { number.clone() };
            format!("{}{}{}", &c[1], escape(&text), &c[4])
        })
        .into_owned();
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r"\documentclass{article}
\begin{document}
\begin{equation}\label{eq:one} a = b \end{equation}
\[ x \]
\begin{align}
  c &= d \label{eq:two} \\
  &= e \notag \\
  f &= g \label{eq:three}
\end{align}
\begin{equation*} y \end{equation*}
% \begin{equation} commented \end{equation}
\begin{equation} h \tag{$\star$} \label{eq:star} \end{equation}
\begin{equation} i \label{eq:four} \end{equation}
\end{document}
";

    #[test]
    fn numbers_follow_latex_rules_across_environments() {
        let n = number(SOURCE);
        assert_eq!(n.labels["eq:one"], "1");
        assert_eq!(n.labels["eq:two"], "2");
        assert_eq!(n.labels["eq:three"], "3");
        assert_eq!(n.labels["eq:star"], "$\\star$");
        assert_eq!(n.labels["eq:four"], "4");
        let shown: Vec<Vec<&str>> = n.blocks.iter().map(|b| b.iter().map(String::as_str).collect()).collect();
        assert_eq!(shown, vec![vec!["1"], vec![], vec!["2", "3"], vec![], vec!["$\\star$"], vec!["4"]]);
    }

    #[test]
    fn rows_split_only_on_top_level_line_breaks() {
        assert_eq!(split_rows(r"a &= \frac{1}{2} \\ b &= \text{x \\ y}"), [r"a &= \frac{1}{2} ", r" b &= \text{x \\ y}"]);
    }

    #[test]
    fn display_math_gets_its_number_and_references_resolve() {
        let n = number("\\begin{document}\n\\begin{equation}\\label{eq:a} a \\end{equation}\n\\[ b \\]\n");
        let html = "<p><math display=\"block\"><mi>a</mi></math></p><p><math display=\"block\"><mi>b</mi></math></p><p>See <a href=\"#eq:a\" data-reference-type=\"eqref\" data-reference=\"eq:a\">[eq:a]</a> and <a href=\"#eq:a\" data-reference-type=\"ref\" data-reference=\"eq:a\">[eq:a]</a>.</p>";
        let out = apply(html, &n);
        assert!(out.contains("<span class=\"eq-number\" id=\"eq:a\">(1)</span><math display=\"block\"><mi>a</mi>"), "{out}");
        assert_eq!(out.matches("eq-number").count(), 1, "the unnumbered block gets nothing: {out}");
        assert!(out.contains(">(1)</a> and"), "{out}");
        assert!(out.contains(">1</a>."), "{out}");
    }

    #[test]
    fn when_the_counts_disagree_only_references_are_fixed() {
        let n = number("\\begin{document}\n\\begin{equation}\\label{eq:a} a \\end{equation}\n");
        let html = "<p>No math here. <a href=\"#eq:a\" data-reference-type=\"eqref\" data-reference=\"eq:a\">[eq:a]</a></p>";
        let out = apply(html, &n);
        assert!(!out.contains("eq-number"));
        assert!(out.contains(">(1)</a>"));
    }

    #[test]
    fn unknown_references_are_left_as_they_are() {
        let html = "<a href=\"#x\" data-reference-type=\"ref\" data-reference=\"sec:intro\">[sec:intro]</a>";
        assert_eq!(apply(html, &Numbering::default()), html);
    }
}
