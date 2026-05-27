use crate::server::domain::comprehension::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSegment {
    pub char_start: u32,
    pub char_end: u32,
}

pub trait TextSegmenter: Send + Sync {
    fn segments(&self, text: &str) -> Vec<TextSegment>;
}

pub fn snap_to_segments(span: Span, segments: &[TextSegment], text_offset: u32) -> Span {
    if segments.is_empty() {
        return span;
    }
    let local_start = span.char_start.saturating_sub(text_offset);
    let local_end = span.char_end.saturating_sub(text_offset);
    let first = segments.iter().find(|s| s.char_end > local_start);
    let last = segments.iter().rev().find(|s| s.char_start < local_end);
    match (first, last) {
        (Some(f), Some(l)) if f.char_start <= l.char_end => Span {
            document_id: span.document_id,
            char_start: text_offset.saturating_add(f.char_start),
            char_end: text_offset.saturating_add(l.char_end),
        },
        _ => span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn seg(start: u32, end: u32) -> TextSegment {
        TextSegment {
            char_start: start,
            char_end: end,
        }
    }

    fn span(start: u32, end: u32) -> Span {
        Span::new(Uuid::nil(), start, end)
    }

    #[test]
    fn returns_original_when_no_segments() {
        let s = span(10, 20);
        assert_eq!(snap_to_segments(s, &[], 0), s);
    }

    #[test]
    fn expands_to_covering_segment() {
        let segments = vec![seg(0, 10), seg(15, 30), seg(35, 50)];
        let snapped = snap_to_segments(span(18, 22), &segments, 0);
        assert_eq!(snapped, span(15, 30));
    }

    #[test]
    fn expands_across_multiple_segments() {
        let segments = vec![seg(0, 10), seg(15, 30), seg(35, 50)];
        let snapped = snap_to_segments(span(8, 40), &segments, 0);
        assert_eq!(snapped, span(0, 50));
    }

    #[test]
    fn respects_text_offset() {
        let segments = vec![seg(0, 10), seg(15, 30)];
        let snapped = snap_to_segments(span(1018, 1022), &segments, 1000);
        assert_eq!(snapped, span(1015, 1030));
    }

    #[test]
    fn snaps_span_starting_before_first_segment() {
        let segments = vec![seg(5, 10), seg(15, 30)];
        let snapped = snap_to_segments(span(0, 8), &segments, 0);
        assert_eq!(snapped, span(5, 10));
    }
}
