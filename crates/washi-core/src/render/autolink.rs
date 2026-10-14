use std::{borrow::Cow, cell::RefCell, ops::Range};

use comrak::{
    nodes::{Ast, AstNode, NodeLink, NodeValue},
    Arena,
};

const SCHEMES: [&str; 2] = ["https://", "http://"];
const TRAILING_PUNCTUATION: &[char] = &['.', ',', ':', ';', '!', '?', '*', '_', '~', '\'', '"'];

fn is_url_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c)
}

pub fn find_urls(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some((start, scheme)) = next_scheme(text, from) {
        let body = start + scheme.len();
        let mut end = body
            + text[body..]
                .find(|c: char| !is_url_char(c))
                .unwrap_or(text.len() - body);
        end = trim_trailing(text, start, end);
        if end > body {
            found.push(start..end);
        }
        from = end.max(body);
    }
    found
}

fn next_scheme(text: &str, from: usize) -> Option<(usize, &'static str)> {
    SCHEMES
        .iter()
        .filter_map(|s| text[from..].find(s).map(|i| (from + i, *s)))
        .filter(|(i, _)| {
            text[..*i]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric())
        })
        .min_by_key(|(i, _)| *i)
}

fn trim_trailing(text: &str, start: usize, mut end: usize) -> usize {
    loop {
        let url = &text[start..end];
        let Some(last) = url.chars().next_back() else {
            return end;
        };
        let unbalanced_paren = last == ')' && url.matches(')').count() > url.matches('(').count();
        if TRAILING_PUNCTUATION.contains(&last) || unbalanced_paren {
            end -= last.len_utf8();
        } else {
            return end;
        }
    }
}

pub fn link_ascii_urls<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>) {
    let texts: Vec<&AstNode> = root
        .descendants()
        .filter(|n| matches!(n.data.borrow().value, NodeValue::Text(_)))
        .filter(|n| !n.ancestors().any(|a| is_link_like(&a.data.borrow().value)))
        .collect();
    for node in texts {
        split_into_links(arena, node);
    }
}

fn is_link_like(value: &NodeValue) -> bool {
    matches!(value, NodeValue::Link(_) | NodeValue::Image(_))
}

fn split_into_links<'a>(arena: &'a Arena<'a>, node: &'a AstNode<'a>) {
    let (text, position) = match &node.data.borrow().value {
        NodeValue::Text(t) => (t.to_string(), node.data.borrow().sourcepos.start),
        _ => return,
    };
    let urls = find_urls(&text);
    if urls.is_empty() {
        return;
    }

    let make = |value: NodeValue| &*arena.alloc(AstNode::new(RefCell::new(Ast::new(value, position))));
    let text_node = |s: &str| make(NodeValue::Text(Cow::Owned(s.to_owned())));

    let mut cursor = 0;
    for range in urls {
        if range.start > cursor {
            node.insert_before(text_node(&text[cursor..range.start]));
        }
        let url = &text[range.clone()];
        let link = make(NodeValue::Link(Box::new(NodeLink {
            url: url.to_owned(),
            title: String::new(),
        })));
        link.append(text_node(url));
        node.insert_before(link);
        cursor = range.end;
    }
    if cursor < text.len() {
        node.insert_before(text_node(&text[cursor..]));
    }
    node.detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(text: &str) -> Vec<&str> {
        find_urls(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn stops_at_non_ascii_text_and_punctuation() {
        assert_eq!(urls("https://example.com、脚注"), ["https://example.com"]);
        assert_eq!(urls("詳細は https://example.com/a/b。次へ"), ["https://example.com/a/b"]);
        assert_eq!(urls("（https://example.com）"), ["https://example.com"]);
    }

    #[test]
    fn trims_trailing_ascii_punctuation() {
        assert_eq!(urls("see https://a.com/x."), ["https://a.com/x"]);
        assert_eq!(urls("see https://a.com/x?, ok"), ["https://a.com/x"]);
    }

    #[test]
    fn keeps_balanced_parentheses_and_query() {
        assert_eq!(urls("https://en.wikipedia.org/wiki/Rust_(language)"), ["https://en.wikipedia.org/wiki/Rust_(language)"]);
        assert_eq!(urls("(https://a.com/x)"), ["https://a.com/x"]);
        assert_eq!(urls("https://a.com/p?q=1&r=2#frag"), ["https://a.com/p?q=1&r=2#frag"]);
    }

    #[test]
    fn finds_several_and_ignores_bare_schemes() {
        assert_eq!(urls("a http://x.io b https://y.io"), ["http://x.io", "https://y.io"]);
        assert!(urls("https:// and nothing").is_empty());
        assert!(urls("ftp://x.io and xhttps://y.io").is_empty());
    }
}
