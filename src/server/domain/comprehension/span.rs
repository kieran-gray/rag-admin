use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_MERGE_GAP: u32 = 32;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Span {
    pub document_id: Uuid,
    pub char_start: u32,
    pub char_end: u32,
}

impl Span {
    pub fn new(document_id: Uuid, char_start: u32, char_end: u32) -> Self {
        Self {
            document_id,
            char_start,
            char_end,
        }
    }

    pub fn len(&self) -> u32 {
        self.char_end.saturating_sub(self.char_start)
    }

    pub fn is_empty(&self) -> bool {
        self.char_end <= self.char_start
    }

    pub fn contains(&self, other: &Span) -> bool {
        self.document_id == other.document_id
            && other.char_start >= self.char_start
            && other.char_end <= self.char_end
    }

    pub fn normalize(spans: impl IntoIterator<Item = Span>) -> Vec<Span> {
        Self::normalize_with_gap(spans, DEFAULT_MERGE_GAP)
    }

    pub fn normalize_with_gap(spans: impl IntoIterator<Item = Span>, max_gap: u32) -> Vec<Span> {
        let mut sorted: Vec<Span> = spans.into_iter().filter(|s| !s.is_empty()).collect();
        sorted.sort_by(|a, b| {
            a.document_id
                .cmp(&b.document_id)
                .then_with(|| a.char_start.cmp(&b.char_start))
                .then_with(|| a.char_end.cmp(&b.char_end))
        });
        let mut out: Vec<Span> = Vec::with_capacity(sorted.len());
        for span in sorted {
            match out.last_mut() {
                Some(last)
                    if last.document_id == span.document_id
                        && span.char_start <= last.char_end.saturating_add(max_gap) =>
                {
                    if span.char_end > last.char_end {
                        last.char_end = span.char_end;
                    }
                }
                _ => out.push(span),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Uuid {
        Uuid::from_u128(0x1111_2222_3333_4444_5555_6666_7777_8888)
    }

    #[test]
    fn merges_adjacent_and_overlapping() {
        let d = doc();
        let spans = vec![
            Span::new(d, 4094, 4138),
            Span::new(d, 4139, 4198),
            Span::new(d, 4213, 4265),
            Span::new(d, 4297, 4391),
            Span::new(d, 4392, 4471),
            Span::new(d, 4472, 4524),
        ];
        let merged = Span::normalize(spans);
        assert_eq!(merged, vec![Span::new(d, 4094, 4524)]);
    }

    #[test]
    fn keeps_separate_when_gap_too_large() {
        let d = doc();
        let spans = vec![Span::new(d, 0, 100), Span::new(d, 200, 300)];
        let merged = Span::normalize(spans);
        assert_eq!(merged, vec![Span::new(d, 0, 100), Span::new(d, 200, 300)]);
    }

    #[test]
    fn dedupes_exact_duplicates() {
        let d = doc();
        let spans = vec![Span::new(d, 10, 20), Span::new(d, 10, 20)];
        let merged = Span::normalize(spans);
        assert_eq!(merged, vec![Span::new(d, 10, 20)]);
    }

    #[test]
    fn keeps_documents_separate() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let spans = vec![Span::new(a, 0, 10), Span::new(b, 0, 10)];
        let merged = Span::normalize(spans);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn drops_empty_spans() {
        let d = doc();
        let spans = vec![Span::new(d, 5, 5), Span::new(d, 10, 20)];
        let merged = Span::normalize(spans);
        assert_eq!(merged, vec![Span::new(d, 10, 20)]);
    }
}
