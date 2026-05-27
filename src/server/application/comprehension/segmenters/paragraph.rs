use crate::server::application::comprehension::ports::{TextSegment, TextSegmenter};

pub struct ParagraphSegmenter;

impl ParagraphSegmenter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ParagraphSegmenter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSegmenter for ParagraphSegmenter {
    fn segments(&self, text: &str) -> Vec<TextSegment> {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut out = Vec::new();
        let mut i = 0;
        while i < len {
            while chars.get(i).is_some_and(|c| c.is_whitespace()) {
                i += 1;
            }
            if i >= len {
                break;
            }
            let start = i;
            while i < len {
                if chars.get(i).copied() == Some('\n') && chars.get(i + 1).copied() == Some('\n') {
                    break;
                }
                i += 1;
            }
            let mut end = i;
            while end > start && chars.get(end - 1).is_some_and(|c| c.is_whitespace()) {
                end -= 1;
            }
            if start < end {
                out.push(TextSegment {
                    char_start: start as u32,
                    char_end: end as u32,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: u32, end: u32) -> TextSegment {
        TextSegment {
            char_start: start,
            char_end: end,
        }
    }

    #[test]
    fn splits_paragraphs_on_blank_lines() {
        let text = "First paragraph.\n\nSecond paragraph here.\n\nThird.";
        let segments = ParagraphSegmenter::new().segments(text);
        assert_eq!(segments, vec![seg(0, 16), seg(18, 40), seg(42, 48)]);
    }

    #[test]
    fn ignores_single_newlines_within_paragraph() {
        let text = "line one\nline two\nline three\n\nnext para";
        let segments = ParagraphSegmenter::new().segments(text);
        assert_eq!(segments.len(), 2);
        assert_eq!(
            &text[..segments[0].char_end as usize],
            "line one\nline two\nline three"
        );
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        let text = "  \n\n  hello  \n\n  ";
        let segments = ParagraphSegmenter::new().segments(text);
        assert_eq!(segments.len(), 1);
        let s = &segments[0];
        let chars: Vec<char> = text.chars().collect();
        let extracted: String = chars[s.char_start as usize..s.char_end as usize]
            .iter()
            .collect();
        assert_eq!(extracted, "hello");
    }

    #[test]
    fn empty_text_yields_no_segments() {
        let segments = ParagraphSegmenter::new().segments("");
        assert!(segments.is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_segments() {
        let segments = ParagraphSegmenter::new().segments("   \n\n  \n\n   ");
        assert!(segments.is_empty());
    }
}
