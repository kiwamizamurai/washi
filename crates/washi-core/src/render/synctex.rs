use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
};

use flate2::read::GzDecoder;

const SP_PER_BP: f64 = 65781.76;
const NEAREST_MAX_BP: f64 = 20.0;

#[derive(Debug, Clone)]
struct Record {
    kind: char,
    tag: u32,
    line: u32,
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    d: i64,
    parent: Option<usize>,
}

impl Record {
    fn is_hbox(&self) -> bool {
        self.kind == '(' || self.kind == 'h'
    }

    fn is_leaf(&self) -> bool {
        matches!(self.kind, 'g' | 'k' | '$' | 'x' | 'r' | 'h') && self.line > 0
    }

    fn contains(&self, x: i64, y: i64) -> bool {
        self.w > 0 && self.x <= x && x <= self.x + self.w && self.y - self.h <= y && y <= self.y + self.d
    }

    fn area(&self) -> i64 {
        self.w.saturating_mul(self.h + self.d)
    }

    fn distance(&self, x: i64, y: i64) -> i64 {
        let (left, right) = (self.x, self.x + self.w.abs());
        let (top, bottom) = (self.y - self.h.abs(), self.y + self.d.abs());
        let horizontal = if x < left { left - x } else if x > right { x - right } else { 0 };
        let vertical = if y < top { top - y } else if y > bottom { y - bottom } else { 0 };
        horizontal + vertical
    }
}

#[derive(Debug, Default)]
pub struct SyncTex {
    inputs: HashMap<u32, String>,
    pages: HashMap<u32, Vec<Record>>,
}

pub struct Hit {
    pub input: String,
    pub line: u32,
}

#[derive(Debug, PartialEq)]
pub struct ForwardHit {
    pub page: u32,
    pub x: f64,
    pub y: f64,
}

impl SyncTex {
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut text = String::new();
        GzDecoder::new(File::open(path).map_err(|e| format!("{}: {e}", path.display()))?)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Ok(Self::parse(&text))
    }

    pub fn parse(text: &str) -> Self {
        let mut synctex = Self::default();
        let mut in_content = false;
        let mut page = 0u32;
        let mut stack: Vec<usize> = Vec::new();

        for line in text.lines() {
            if !in_content {
                if let Some(rest) = line.strip_prefix("Input:") {
                    if let Some((tag, name)) = rest.split_once(':') {
                        if let Ok(tag) = tag.parse() {
                            synctex.inputs.insert(tag, name.to_owned());
                        }
                    }
                } else if line == "Content:" {
                    in_content = true;
                }
                continue;
            }

            let mut chars = line.chars();
            let Some(kind) = chars.next() else { continue };
            let rest = chars.as_str();
            match kind {
                '{' => {
                    page = rest.parse().unwrap_or(0);
                    stack.clear();
                }
                '}' => stack.clear(),
                ']' | ')' => {
                    stack.pop();
                }
                '[' | '(' | 'h' | 'v' | 'k' | 'g' | '$' | 'x' | 'r' => {
                    let Some(mut record) = parse_record(kind, rest) else { continue };
                    record.parent = stack.last().copied();
                    let records = synctex.pages.entry(page).or_default();
                    records.push(record);
                    if kind == '[' || kind == '(' {
                        stack.push(records.len() - 1);
                    }
                }
                _ => {}
            }
        }
        synctex
    }

    pub fn forward(&self, matches: impl Fn(&str) -> bool, line: u32) -> Option<ForwardHit> {
        let tags: HashSet<u32> = self
            .inputs
            .iter()
            .filter(|(_, name)| !name.is_empty() && matches(name))
            .map(|(tag, _)| *tag)
            .collect();
        self.pages
            .iter()
            .flat_map(|(page, records)| records.iter().map(move |r| (*page, r)))
            .filter(|(_, r)| r.line > 0 && tags.contains(&r.tag) && (r.is_leaf() || r.is_hbox()))
            .min_by_key(|(page, r)| (r.line.abs_diff(line), *page, r.y - r.h.max(0), r.x))
            .map(|(page, r)| ForwardHit {
                page,
                x: r.x as f64 / SP_PER_BP,
                y: (r.y - r.h.max(0)) as f64 / SP_PER_BP,
            })
    }

    pub fn inverse(&self, page: u32, x_bp: f64, y_bp: f64) -> Option<Hit> {
        let records = self.pages.get(&page)?;
        let (x, y) = ((x_bp * SP_PER_BP) as i64, (y_bp * SP_PER_BP) as i64);

        let inside = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_hbox() && r.contains(x, y))
            .min_by_key(|(_, r)| r.area());
        let nearest = || {
            let limit = (NEAREST_MAX_BP * SP_PER_BP) as i64;
            records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.is_hbox() && r.w > 0)
                .map(|(i, r)| (r.distance(x, y), r.area(), i))
                .filter(|(distance, _, _)| *distance <= limit)
                .min()
                .map(|(_, _, i)| i)
        };
        let container = inside.map(|(i, _)| i).or_else(nearest)?;

        let descendants = records
            .iter()
            .enumerate()
            .filter(|(i, r)| r.is_leaf() && *i != container && is_inside(records, *i, container));
        let (before, after): (Vec<_>, Vec<_>) = descendants.partition(|(_, r)| r.x <= x);
        let leaf = before
            .into_iter()
            .max_by_key(|(_, r)| r.x)
            .or_else(|| after.into_iter().min_by_key(|(_, r)| r.x))
            .map(|(_, r)| r)
            .or(Some(&records[container]).filter(|r| r.line > 0))?;

        let input = self.inputs.get(&leaf.tag).filter(|name| !name.is_empty())?;
        Some(Hit { input: input.clone(), line: leaf.line })
    }
}

fn is_inside(records: &[Record], mut index: usize, ancestor: usize) -> bool {
    while let Some(parent) = records[index].parent {
        if parent == ancestor {
            return true;
        }
        index = parent;
    }
    false
}

fn parse_record(kind: char, rest: &str) -> Option<Record> {
    let (head, tail) = rest.split_once(':')?;
    let (tag, line) = head.split_once(',')?;
    let mut sections = tail.split(':');
    let (x, y) = sections.next()?.split_once(',')?;
    let size: Vec<i64> = sections
        .next()
        .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default();
    let at = |i: usize| size.get(i).copied().unwrap_or(0);
    Some(Record {
        kind,
        tag: tag.parse().ok()?,
        line: line.parse().ok()?,
        x: x.parse().ok()?,
        y: y.parse().ok()?,
        w: at(0),
        h: if kind == 'k' { 0 } else { at(1) },
        d: if kind == 'k' { 0 } else { at(2) },
        parent: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "SyncTeX Version:1\nInput:1:/doc/a.tex\nInput:2:\nOutput:pdf\nMagnification:1000\nUnit:1\nX Offset:0\nY Offset:0\nContent:\n!100\n{1\n[1,68:4736287,52685372:29760291,47949085,0\n(1,10:4736287,8000000:29760291,800000,200000\nk1,10:4736287,8000000:100000\ng1,10:9000000,8000000\ng1,12:20000000,8000000\n)\n(1,30:4736287,12000000:29760291,800000,200000\ng1,31:6000000,12000000\n)\n]\n}1\n{2\n(1,40:4736287,9000000:29760291,800000,200000\ng1,41:7000000,9000000\n)\n}2\n";

    fn bp(sp: i64) -> f64 {
        sp as f64 / SP_PER_BP
    }

    #[test]
    fn parses_inputs_and_pages() {
        let s = SyncTex::parse(SAMPLE);
        assert_eq!(s.inputs.get(&1).map(String::as_str), Some("/doc/a.tex"));
        assert_eq!(s.pages.len(), 2);
    }

    #[test]
    fn picks_the_leaf_at_or_before_the_click_within_the_line_box() {
        let s = SyncTex::parse(SAMPLE);
        let near_first = s.inverse(1, bp(9_500_000), bp(8_000_000)).unwrap();
        assert_eq!((near_first.input.as_str(), near_first.line), ("/doc/a.tex", 10));
        let near_second = s.inverse(1, bp(25_000_000), bp(8_000_000)).unwrap();
        assert_eq!(near_second.line, 12);
    }

    #[test]
    fn chooses_the_innermost_line_box_vertically() {
        let s = SyncTex::parse(SAMPLE);
        assert_eq!(s.inverse(1, bp(7_000_000), bp(12_000_000)).unwrap().line, 31);
        assert_eq!(s.inverse(2, bp(8_000_000), bp(9_000_000)).unwrap().line, 41);
    }

    #[test]
    fn a_click_just_outside_a_line_box_still_finds_the_nearest_line() {
        let s = SyncTex::parse(SAMPLE);
        let below_first = s.inverse(1, bp(9_000_000), bp(8_200_000 + 600_000)).unwrap();
        assert_eq!(below_first.line, 10, "9 bp under the first line belongs to it");
        let above_second = s.inverse(1, bp(6_000_000), bp(11_200_000 - 500_000)).unwrap();
        assert_eq!(above_second.line, 31, "closer to the second line than to the first");
        let left_of_first = s.inverse(1, bp(4_000_000), bp(7_900_000)).unwrap();
        assert_eq!(left_of_first.line, 10, "to the left of the line, at its height");
    }

    #[test]
    fn the_nearest_line_is_not_chosen_when_the_click_is_far_from_every_line() {
        let s = SyncTex::parse(SAMPLE);
        assert!(s.inverse(1, bp(9_000_000), bp(9_700_000)).is_none(), "midway between two lines, 23 bp from each");
        assert!(s.inverse(1, bp(40_000_000), bp(8_000_000)).is_none(), "far into the right margin");
    }

    #[test]
    fn a_click_inside_a_line_box_is_unchanged_by_the_nearest_rule() {
        let s = SyncTex::parse(SAMPLE);
        let hit = s.inverse(1, bp(20_000_000), bp(8_000_000)).unwrap();
        assert_eq!(hit.line, 12);
    }

    #[test]
    fn misses_outside_any_line_box_or_on_unknown_pages() {
        let s = SyncTex::parse(SAMPLE);
        assert!(s.inverse(1, bp(7_000_000), bp(30_000_000)).is_none());
        assert!(s.inverse(9, bp(7_000_000), bp(8_000_000)).is_none());
    }

    #[test]
    fn ignores_records_whose_input_name_is_empty() {
        let text = SAMPLE.replace("g1,12:20000000", "g2,12:20000000");
        let s = SyncTex::parse(&text);
        let hit = s.inverse(1, bp(25_000_000), bp(8_000_000));
        assert!(hit.is_none());
    }

    fn is_a_tex(name: &str) -> bool {
        name == "/doc/a.tex"
    }

    #[test]
    fn forward_finds_the_page_and_the_top_of_the_line() {
        let s = SyncTex::parse(SAMPLE);
        let hit = s.forward(is_a_tex, 10).unwrap();
        assert_eq!(hit.page, 1);
        assert!((hit.x - bp(4_736_287)).abs() < 1e-6, "{hit:?}");
        assert!((hit.y - bp(8_000_000 - 800_000)).abs() < 1e-6, "{hit:?}");
        assert_eq!(s.forward(is_a_tex, 40).unwrap().page, 2);
    }

    #[test]
    fn forward_uses_the_nearest_line_when_the_exact_one_has_no_box() {
        let s = SyncTex::parse(SAMPLE);
        assert_eq!(s.forward(is_a_tex, 11).unwrap().page, 1);
        assert_eq!(s.forward(is_a_tex, 100).unwrap().page, 2);
        assert_eq!(s.forward(is_a_tex, 1).unwrap().page, 1);
    }

    #[test]
    fn forward_ignores_other_input_files() {
        let s = SyncTex::parse(SAMPLE);
        assert!(s.forward(|name| name == "/doc/other.tex", 10).is_none());
    }
}
