use async_trait::async_trait;

use crate::server::application::chunking::{ChunkOutput, DocumentChunker, TokenBudget};
use crate::server::application::markdown::{Document, SegmentBlock};
use crate::server::application::ports::Tokenizer;
use crate::server::application::AppError;
use crate::shared::{ChunkStrategy, ChunkingConfig};

pub struct BertChunker;

#[async_trait]
impl DocumentChunker for BertChunker {
    fn strategy(&self) -> ChunkStrategy {
        ChunkStrategy::Bert
    }

    async fn chunk(
        &self,
        config: &ChunkingConfig,
        source: &Document,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<ChunkOutput>, AppError> {
        let ChunkingConfig::Bert(config) = config else {
            unreachable!()
        };

        let target = config.target_tokens.max(1) as usize;
        let overlap = config.overlap_tokens as usize;
        let min = config.min_tokens as usize;
        let budget = TokenBudget::new(tokenizer);

        let raw = source.bert_segments();
        let packed = pack_segments(raw, target, &budget)?;
        let split = split_oversized(packed, target, overlap, min, &budget)?;

        Ok(split
            .into_iter()
            .filter(|s| !s.text.trim().is_empty())
            .enumerate()
            .map(|(i, s)| ChunkOutput {
                chunk_id: i as u32,
                heading: s.heading,
                text: s.text.trim().to_string(),
                char_start: s.char_start as u32,
                char_end: s.char_end as u32,
            })
            .collect())
    }
}

fn pack_segments(
    segments: Vec<SegmentBlock>,
    target: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<SegmentBlock>, AppError> {
    let mut packed = Vec::new();
    let mut current: Option<SegmentBlock> = None;

    for seg in segments {
        let cur = match current.take() {
            None => {
                current = Some(seg);
                continue;
            }
            Some(c) => c,
        };

        if cur.atomic || seg.atomic || cur.heading != seg.heading {
            packed.push(cur);
            current = Some(seg);
            continue;
        }

        let merged_text = format!("{}\n{}", cur.text, seg.text);
        if budget.count_str(&merged_text)? <= target {
            current = Some(SegmentBlock {
                text: merged_text,
                char_start: cur.char_start,
                char_end: seg.char_end,
                heading: cur.heading,
                atomic: cur.atomic,
            });
            continue;
        }

        packed.push(cur);
        current = Some(seg);
    }

    if let Some(c) = current {
        packed.push(c);
    }
    Ok(packed)
}

fn last_double_newline(window: &[char]) -> Option<usize> {
    if window.len() < 2 {
        return None;
    }
    let mut i = window.len() - 2;
    loop {
        if window[i] == '\n' && window[i + 1] == '\n' {
            return Some(i);
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

fn last_sentence_break(window: &[char]) -> Option<usize> {
    let mut last_punct: Option<usize> = None;
    for (i, &c) in window.iter().enumerate() {
        if c == '.' || c == '!' || c == '?' {
            last_punct = Some(i);
        }
    }
    let i = last_punct?;
    if i + 1 < window.len() && window[i + 1].is_whitespace() {
        Some(i)
    } else {
        None
    }
}

fn split_oversized(
    segments: Vec<SegmentBlock>,
    target: usize,
    overlap: usize,
    min: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<SegmentBlock>, AppError> {
    let mut out = Vec::new();
    for seg in segments {
        let text_chars: Vec<char> = seg.text.chars().collect();
        if budget.count_chars(&text_chars)? <= target {
            out.push(seg);
            continue;
        }

        let mut start = 0usize;
        let total = text_chars.len();
        loop {
            if start >= total {
                break;
            }
            let prefix_len = budget.max_prefix_chars(&text_chars[start..], target)?;
            let end = (start + prefix_len).min(total);
            let mut break_at = end;
            if end < total {
                let window = &text_chars[start..end];
                let last_para = last_double_newline(window);
                let last_sent = last_sentence_break(window);
                if let Some(p) = last_para {
                    if p > prefix_len / 2 {
                        break_at = start + p;
                    } else if let Some(s) = last_sent {
                        if s > prefix_len / 2 {
                            break_at = start + s + 1;
                        }
                    }
                } else if let Some(s) = last_sent {
                    if s > prefix_len / 2 {
                        break_at = start + s + 1;
                    }
                }
            }
            let raw_piece: String = text_chars[start..break_at].iter().collect();
            let piece = raw_piece.trim().to_string();
            let piece_len = budget.count_str(&piece)?;
            if !piece.is_empty() {
                let last_atomic = out.last().map(|s: &SegmentBlock| s.atomic).unwrap_or(true);
                if piece_len < min && !out.is_empty() && !last_atomic {
                    let prev = out.last_mut().unwrap();
                    let merged = format!("{}\n\n{}", prev.text, piece);
                    if budget.count_str(&merged)? <= target {
                        prev.text = merged;
                        prev.char_end = seg.char_start + break_at;
                    } else {
                        out.push(SegmentBlock {
                            text: piece,
                            char_start: seg.char_start + start,
                            char_end: seg.char_start + break_at,
                            heading: seg.heading.clone(),
                            atomic: false,
                        });
                    }
                } else {
                    out.push(SegmentBlock {
                        text: piece,
                        char_start: seg.char_start + start,
                        char_end: seg.char_start + break_at,
                        heading: seg.heading.clone(),
                        atomic: false,
                    });
                }
            }
            if break_at >= total {
                break;
            }
            let candidate = budget.suffix_start_for_overlap(&text_chars, break_at, overlap)?;
            start = if candidate > start {
                candidate
            } else {
                break_at
            };
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::application::ports::MarkdownParser;
    use crate::server::application::test_support::MockTokenizer;
    use crate::server::infrastructure::markdown::MarkdownRsParser;
    use crate::shared::BertChunkingConfig;

    fn chunker() -> BertChunker {
        BertChunker {}
    }

    fn to_markdown(source: &str) -> Document {
        let markdown_parser = MarkdownRsParser {};
        markdown_parser.parse(source).unwrap()
    }

    fn cfg() -> ChunkingConfig {
        ChunkingConfig::Bert(BertChunkingConfig::default())
    }

    #[tokio::test]
    async fn chunk_yields_sequential_ids() {
        let chunker = chunker();
        let tokenizer = MockTokenizer::new();

        let src = &to_markdown("## A\nfirst paragraph.\n\n## B\nsecond paragraph.");
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        assert!(!chunks.is_empty());
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.chunk_id as usize, i);
        }
    }

    #[tokio::test]
    async fn chunk_preserves_heading_path() {
        let chunker = chunker();
        let tokenizer = MockTokenizer::new();

        let src = &to_markdown("# Top\n\nintro\n\n## Sub\n\ndetail");
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        let headings: Vec<&str> = chunks.iter().map(|c| c.heading.as_str()).collect();
        assert!(headings.iter().any(|h| h.contains("Top")));
        assert!(headings.iter().any(|h| h.contains("Sub")));
    }

    #[tokio::test]
    async fn chunk_does_not_split_inside_fence() {
        let chunker = chunker();
        let tokenizer = MockTokenizer::new();

        let code: String = (0..20)
            .map(|i| format!("let x{i} = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let src = &to_markdown(&format!("## Code\n\n```rust\n{code}\n```\n"));
        let chunks = chunker.chunk(&cfg(), &src, &tokenizer).await.unwrap();
        for c in chunks.iter().filter(|c| c.text.contains("```")) {
            let opens = c.text.matches("```").count();
            assert_eq!(opens % 2, 0, "chunk {} has unbalanced fences", c.chunk_id);
        }
    }

    #[tokio::test]
    async fn smaller_target_yields_more_chunks() {
        let chunker = chunker();
        let tokenizer = MockTokenizer::new();

        let para = "Lorem ipsum dolor sit amet. ".repeat(120);
        let src = &to_markdown(&format!("## Big\n\n{para}"));
        let big = ChunkingConfig::Bert(BertChunkingConfig {
            target_tokens: 384,
            overlap_tokens: 64,
            min_tokens: 96,
        });
        let small = ChunkingConfig::Bert(BertChunkingConfig {
            target_tokens: 128,
            overlap_tokens: 16,
            min_tokens: 32,
        });
        let big_chunks = chunker.chunk(&big, &src, &tokenizer).await.unwrap();
        let small_chunks = chunker.chunk(&small, &src, &tokenizer).await.unwrap();
        assert!(small_chunks.len() > big_chunks.len());
    }
}
