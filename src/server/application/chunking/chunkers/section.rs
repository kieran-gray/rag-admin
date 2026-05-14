use async_trait::async_trait;

use crate::{
    server::application::{
        chunking::{ChunkOutput, DocumentChunker, TokenBudget},
        markdown::{Document, SectionBlock},
        ports::Tokenizer,
        AppError,
    },
    shared::{ChunkStrategy, ChunkingConfig},
};

const SECTION_CUT_DEPTH: usize = 3;

pub struct SectionChunker;

#[async_trait]
impl DocumentChunker for SectionChunker {
    fn strategy(&self) -> ChunkStrategy {
        ChunkStrategy::Section
    }

    async fn chunk(
        &self,
        config: &ChunkingConfig,
        source: &Document,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<ChunkOutput>, AppError> {
        let ChunkingConfig::Section(config) = config else {
            unreachable!()
        };

        let budget = TokenBudget::new(tokenizer);
        let sections = source.sections(SECTION_CUT_DEPTH);
        let mut split = Vec::new();

        for section in sections {
            split.extend(split_oversized(
                section,
                config.max_section_tokens as usize,
                &budget,
            )?);
        }

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

fn split_oversized(
    section: SectionBlock,
    max_tokens: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<SectionBlock>, AppError> {
    let chars: Vec<char> = section.text.chars().collect();
    if budget.count_chars(&chars)? <= max_tokens {
        return Ok(vec![section]);
    }

    let mut out = Vec::new();
    let total = chars.len();
    let mut start = 0usize;
    while start < total {
        let prefix_len = budget.max_prefix_chars(&chars[start..], max_tokens)?;
        let end = (start + prefix_len).min(total);
        let break_at = if end < total {
            last_double_newline(&chars[start..end])
                .map(|p| start + p)
                .unwrap_or(end)
        } else {
            end
        };
        let break_at = if break_at <= start { end } else { break_at };
        let piece: String = chars[start..break_at].iter().collect();
        out.push(SectionBlock {
            text: piece,
            char_start: section.char_start + start,
            char_end: section.char_start + break_at,
            heading: section.heading.clone(),
        });
        start = break_at;
        while start < total && chars[start] == '\n' {
            start += 1;
        }
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use crate::{
        server::{application::ports::MarkdownParser, infrastructure::markdown::MarkdownRsParser},
        shared::SectionChunkingConfig,
    };

    use super::*;

    fn chunker() -> SectionChunker {
        SectionChunker {}
    }

    fn to_markdown(source: &str) -> Document {
        let markdown_parser = MarkdownRsParser {};
        markdown_parser.parse(source).unwrap()
    }

    fn cfg() -> ChunkingConfig {
        ChunkingConfig::Section(SectionChunkingConfig::default())
    }

    #[tokio::test]
    async fn one_chunk_per_h2() {
        let src = &to_markdown("## A\n\nfirst paragraph.\n\n## B\n\nsecond paragraph.");
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.contains("first paragraph"));
        assert!(chunks[1].text.contains("second paragraph"));
    }

    #[tokio::test]
    async fn h4_does_not_cut() {
        let src = &to_markdown("## Top\n\nintro\n\n#### Deep\n\nstill same chunk.");
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("Deep"));
        assert!(chunks[0].text.contains("still same chunk"));
    }

    #[tokio::test]
    async fn fenced_heading_does_not_cut() {
        let src = &to_markdown("## A\n\n```md\n## not a real heading\n```\n\nbody");
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("## not a real heading"));
    }

    #[tokio::test]
    async fn oversized_section_splits() {
        let ChunkingConfig::Section(config) = cfg() else {
            unreachable!()
        };

        let max = config.max_section_tokens as usize;
        let para = "x ".repeat(max / 2);
        let src = &to_markdown(&format!("## Big\n\n{para}\n\n{para}\n\n{para}"));
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), &src, &tokenizer).await.unwrap();
        assert!(
            chunks.len() > 1,
            "expected fallback split, got {} chunks",
            chunks.len()
        );
        for c in &chunks {
            assert_eq!(c.heading, "Big");
        }
    }

    #[tokio::test]
    async fn smaller_max_section_tokens_increases_splits() {
        let chunker = chunker();

        let para = "x ".repeat(300);
        let src = &to_markdown(&format!("## Big\n\n{para}\n\n{para}\n\n{para}"));

        let big = ChunkingConfig::Section(SectionChunkingConfig {
            max_section_tokens: 480,
        });
        let small = ChunkingConfig::Section(SectionChunkingConfig {
            max_section_tokens: 128,
        });

        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let big_chunks = chunker.chunk(&big, &src, &tokenizer).await.unwrap();
        let small_chunks = chunker.chunk(&small, &src, &tokenizer).await.unwrap();
        assert!(small_chunks.len() > big_chunks.len());
    }

    #[tokio::test]
    async fn heading_path_preserved() {
        let src = &to_markdown("# Top\n\nintro\n\n## Sub\n\ndetail");
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        let headings: Vec<&str> = chunks.iter().map(|c| c.heading.as_str()).collect();
        assert!(headings.iter().any(|h| h.contains("Top")));
        assert!(headings.iter().any(|h| h.contains("Sub")));
        assert!(headings.iter().any(|h| h.contains("Top > Sub")));
    }

    #[tokio::test]
    async fn skipped_heading_levels() {
        let src = &to_markdown("# Top\n\n### Deep");
        let chunker = chunker();
        let tokenizer = crate::server::application::test_support::MockTokenizer::new();
        let chunks = chunker.chunk(&cfg(), src, &tokenizer).await.unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].heading, "Top");
        assert_eq!(chunks[1].heading, "Top > Deep");
    }
}
