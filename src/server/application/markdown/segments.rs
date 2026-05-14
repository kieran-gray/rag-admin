use super::{heading_path, Block, Document, SegmentBlock};

impl Document {
    pub fn bert_segments(&self) -> Vec<SegmentBlock> {
        let mut segments = Vec::new();
        let mut heading_path: Vec<String> = Vec::new();
        let mut current: Vec<&Block> = Vec::new();

        for block in &self.blocks {
            if let Some(heading) = block.heading() {
                flush_segment(&mut segments, &current, &heading_path);
                current.clear();
                heading_path::update_heading_path(&mut heading_path, heading);
                current.push(block);
                continue;
            }

            if block.is_atomic_segment() {
                flush_segment(&mut segments, &current, &heading_path);
                current.clear();
                segments.push(SegmentBlock {
                    text: block.text.clone(),
                    char_start: block.span.char_start,
                    char_end: block.span.char_end,
                    heading: heading_path::join_heading_path(&heading_path),
                    atomic: true,
                });
                continue;
            }

            current.push(block);
        }

        flush_segment(&mut segments, &current, &heading_path);
        segments
    }
}

fn flush_segment(out: &mut Vec<SegmentBlock>, current: &[&Block], heading_path: &[String]) {
    let Some(first) = current.first() else {
        return;
    };
    let last = current.last().unwrap();
    out.push(SegmentBlock {
        text: heading_path::collect_block_text(current),
        char_start: first.span.char_start,
        char_end: last.span.char_end,
        heading: heading_path::join_heading_path(heading_path),
        atomic: false,
    });
}

#[cfg(test)]
mod tests {
    use crate::server::application::ports::MarkdownParser;
    use crate::server::infrastructure::markdown::MarkdownRsParser;

    #[test]
    fn bert_segments_keep_code_fences_atomic() {
        let parser = MarkdownRsParser;
        let doc = parser
            .parse("## Code\nIntro\n\n```rust\nfn main() {}\n```\n\nOutro\n")
            .unwrap();

        let segments = doc.bert_segments();

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].heading, "Code");
        assert!(!segments[0].atomic);
        assert!(segments[1].atomic);
        assert!(segments[1].text.contains("```rust"));
        assert_eq!(segments[2].heading, "Code");
        assert!(segments[2].text.contains("Outro"));
    }
}
