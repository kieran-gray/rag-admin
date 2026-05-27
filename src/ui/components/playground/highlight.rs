use std::str;

use leptos::prelude::*;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn HighlightedSnippet(
    #[prop(into)] text: String,
    #[prop(into)] query: String,
) -> impl IntoView {
    let segments = highlight_segments(&text, &query);
    view! {
        <p class="playground-hit-snippet">
            {segments.into_iter().map(|seg| match seg {
                Segment::Plain(s) => view! { <span>{s}</span> }.into_any(),
                Segment::Match(s) => view! { <mark class="snippet-mark">{s}</mark> }.into_any(),
            }).collect_view()}
        </p>
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Plain(String),
    Match(String),
}

pub fn highlight_segments(text: &str, query: &str) -> Vec<Segment> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|t| t.chars().count() >= 2)
        .map(str::to_lowercase)
        .collect();
    if terms.is_empty() {
        return vec![Segment::Plain(text.to_string())];
    }

    let lower = text.to_lowercase();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in &terms {
        let term_len = term.len();
        if term_len == 0 {
            continue;
        }
        let mut start = 0_usize;
        while let Some(haystack) = lower.get(start..) {
            let Some(idx) = haystack.find(term.as_str()) else {
                break;
            };
            let pos = start + idx;
            let end = pos + term_len;
            ranges.push((pos, end));
            start = end;
        }
    }

    if ranges.is_empty() {
        return vec![Segment::Plain(text.to_string())];
    }

    ranges.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for r in ranges {
        if let Some(last) = merged.last_mut() {
            if r.0 <= last.1 {
                if r.1 > last.1 {
                    last.1 = r.1;
                }
                continue;
            }
        }
        merged.push(r);
    }

    let bytes = text.as_bytes();
    let mut out: Vec<Segment> = Vec::with_capacity(merged.len() * 2 + 1);
    let mut cursor = 0;
    for (start, end) in merged {
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            continue;
        }
        if start > cursor {
            if let Some(slice) = bytes.get(cursor..start) {
                if let Ok(s) = str::from_utf8(slice) {
                    out.push(Segment::Plain(s.to_string()));
                }
            }
        }
        if let Some(slice) = bytes.get(start..end) {
            if let Ok(s) = str::from_utf8(slice) {
                out.push(Segment::Match(s.to_string()));
            }
        }
        cursor = end;
    }
    if cursor < text.len() {
        if let Some(slice) = bytes.get(cursor..) {
            if let Ok(s) = str::from_utf8(slice) {
                out.push(Segment::Plain(s.to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_case_insensitive_matches() {
        let segments = highlight_segments("The Quick Brown fox", "quick FOX");
        assert_eq!(
            segments,
            vec![
                Segment::Plain("The ".to_string()),
                Segment::Match("Quick".to_string()),
                Segment::Plain(" Brown ".to_string()),
                Segment::Match("fox".to_string()),
            ]
        );
    }

    #[test]
    fn skips_short_terms() {
        let segments = highlight_segments("a apple", "a apple");
        assert_eq!(
            segments,
            vec![
                Segment::Plain("a ".to_string()),
                Segment::Match("apple".to_string()),
            ]
        );
    }

    #[test]
    fn returns_plain_when_no_match() {
        let segments = highlight_segments("hello", "xyz");
        assert_eq!(segments, vec![Segment::Plain("hello".to_string())]);
    }

    #[test]
    fn merges_overlapping_ranges() {
        let segments = highlight_segments("abcabc", "abc abca");
        // "abca" overlaps "abc" at 0..3 and 3..6 -> merged into 0..6
        assert_eq!(segments.len(), 1);
        assert!(matches!(&segments[0], Segment::Match(s) if s == "abcabc"));
    }
}
