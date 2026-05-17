use async_trait::async_trait;

use crate::catalog::ChunkStrategy;
use crate::core::{ChunkingConfig, DarnChunkingConfig, DarnGranularity};
use crate::server::application::chunking::{ChunkOutput, DocumentChunker};
use crate::server::application::markdown::Document;
use crate::server::application::ports::Tokenizer;
use crate::server::application::AppError;

use super::chunk_optimiser::{ChunkOptimiser, Granularity};
use super::md_parser::MdParser;
use super::rule_manager::RuleManager;

pub struct DarnChunker;

#[async_trait]
impl DocumentChunker for DarnChunker {
    fn strategy(&self) -> ChunkStrategy {
        ChunkStrategy::Darn
    }

    async fn chunk(
        &self,
        config: &ChunkingConfig,
        source: &Document,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<ChunkOutput>, AppError> {
        let ChunkingConfig::Darn(config) = config else {
            return Err(AppError::Validation(
                "Darn chunker called with invalid chunking config".to_string(),
            ));
        };

        let text = source.source.as_str();
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let cut_indices = compute_cut_indices(text, config, tokenizer)?;
        let chunks = assemble_chunks(text, &cut_indices, config);

        Ok(chunks
            .into_iter()
            .filter(|c| !c.text.trim().is_empty())
            .enumerate()
            .map(|(i, c)| ChunkOutput {
                chunk_id: i as u32,
                heading: c.heading,
                text: c.text.trim().to_string(),
                char_start: c.char_start as u32,
                char_end: c.char_end as u32,
            })
            .collect())
    }
}

struct RawChunk {
    text: String,
    heading: String,
    char_start: usize,
    char_end: usize,
}

fn compute_cut_indices(
    text: &str,
    config: &DarnChunkingConfig,
    tokenizer: &dyn Tokenizer,
) -> Result<Vec<usize>, AppError> {
    let ranges = MdParser::parse(text)?;
    let punishments = RuleManager::build_punishment_vector(&ranges, text.len(), None);
    let granularity = match config.granularity {
        DarnGranularity::Characters => Granularity::Characters,
        DarnGranularity::Tokens => Granularity::Tokens,
    };
    let optimiser = ChunkOptimiser::new(text, punishments, granularity, tokenizer)?;
    let max = (config.max_chunk_size as usize).max(1);
    optimiser.optimise_chunks(max)
}

fn assemble_chunks(
    text: &str,
    cut_indices: &[usize],
    config: &DarnChunkingConfig,
) -> Vec<RawChunk> {
    if cut_indices.is_empty() {
        return Vec::new();
    }

    let text_len = text.len();
    let overlap = config.overlap as usize;
    let mut out = Vec::with_capacity(cut_indices.len());

    for (i, &start_byte) in cut_indices.iter().enumerate() {
        let next_start = cut_indices.get(i + 1).copied().unwrap_or(text_len);
        let base_end = next_start;

        let end_with_overlap = if overlap > 0 && i + 1 < cut_indices.len() {
            extend_with_overlap(text, base_end, overlap, config)
        } else {
            base_end
        };

        let safe_start = floor_char_boundary(text, start_byte);
        let safe_end = ceil_char_boundary(text, end_with_overlap);
        if safe_end <= safe_start {
            continue;
        }

        let Some(slice) = text.get(safe_start..safe_end) else {
            continue;
        };

        out.push(RawChunk {
            text: slice.to_string(),
            heading: extract_heading(slice),
            char_start: text.get(..safe_start).map_or(0, |s| s.chars().count()),
            char_end: text.get(..safe_end).map_or(0, |s| s.chars().count()),
        });
    }

    out
}

fn extend_with_overlap(
    text: &str,
    base_end: usize,
    overlap: usize,
    config: &DarnChunkingConfig,
) -> usize {
    match config.granularity {
        DarnGranularity::Characters => {
            let suffix = text.get(base_end..).unwrap_or("");
            let mut count = 0;
            let mut end = base_end;
            for (i, _) in suffix.char_indices() {
                if count >= overlap {
                    break;
                }
                end = base_end + i;
                count += 1;
            }
            if count < overlap {
                text.len()
            } else {
                end
            }
        }
        DarnGranularity::Tokens => {
            // The optimiser is the source of truth for token boundaries, but we
            // don't have it here. Fall back to a character-based extension that
            // pulls in `overlap * 4` chars — a rough approximation that mirrors
            // typical token→char ratios. Safe because `ceil_char_boundary` keeps
            // us aligned with UTF-8.
            extend_with_overlap(
                text,
                base_end,
                overlap.saturating_mul(4),
                &DarnChunkingConfig {
                    max_chunk_size: config.max_chunk_size,
                    overlap: config.overlap,
                    granularity: DarnGranularity::Characters,
                },
            )
        }
    }
}

fn floor_char_boundary(text: &str, mut byte: usize) -> usize {
    if byte > text.len() {
        return text.len();
    }
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn ceil_char_boundary(text: &str, mut byte: usize) -> usize {
    if byte >= text.len() {
        return text.len();
    }
    while byte < text.len() && !text.is_char_boundary(byte) {
        byte += 1;
    }
    byte
}

fn extract_heading(text: &str) -> String {
    for line in text.lines() {
        if let Some((_depth, heading)) = parse_atx_heading(line) {
            return heading;
        }
    }
    String::new()
}

fn parse_atx_heading(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    while bytes.get(depth).copied() == Some(b'#') {
        depth += 1;
    }
    if depth == 0 || depth > 6 {
        return None;
    }
    let after = line.get(depth..)?;
    let first = after.chars().next()?;
    if !first.is_whitespace() {
        return None;
    }
    let trimmed = after.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some((depth, trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DarnChunkingConfig;
    use crate::server::application::ports::MarkdownParser;
    use crate::server::application::test_support::MockTokenizer;
    use crate::server::infrastructure::markdown::MarkdownRsParser;

    fn chunker() -> DarnChunker {
        DarnChunker
    }

    fn to_doc(source: &str) -> Document {
        MarkdownRsParser.parse(source).unwrap()
    }

    fn chars_cfg(max: u32, overlap: u32) -> ChunkingConfig {
        ChunkingConfig::Darn(DarnChunkingConfig {
            max_chunk_size: max,
            overlap,
            granularity: DarnGranularity::Characters,
        })
    }

    #[tokio::test]
    async fn empty_input_yields_no_chunks() {
        let tk = MockTokenizer::new();
        let doc = to_doc("");
        let chunks = chunker()
            .chunk(&chars_cfg(500, 50), &doc, &tk)
            .await
            .unwrap();
        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn sequential_ids() {
        let tk = MockTokenizer::new();
        let body = "# Heading\n\n".to_string() + &"A long paragraph. ".repeat(80);
        let doc = to_doc(&body);
        let chunks = chunker()
            .chunk(&chars_cfg(200, 0), &doc, &tk)
            .await
            .unwrap();
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.chunk_id as usize, i);
        }
    }

    #[tokio::test]
    async fn smaller_max_size_yields_more_chunks() {
        let tk = MockTokenizer::new();
        let body = "Lorem ipsum dolor sit amet. ".repeat(120);
        let doc = to_doc(&body);
        let big = chunker()
            .chunk(&chars_cfg(800, 0), &doc, &tk)
            .await
            .unwrap();
        let small = chunker()
            .chunk(&chars_cfg(200, 0), &doc, &tk)
            .await
            .unwrap();
        assert!(
            small.len() > big.len(),
            "expected smaller chunk_size to yield more chunks; got big={} small={}",
            big.len(),
            small.len()
        );
    }

    #[tokio::test]
    async fn respects_max_chunk_size_in_characters() {
        let tk = MockTokenizer::new();
        let body = "alpha beta gamma delta. ".repeat(60);
        let doc = to_doc(&body);
        let max = 250u32;
        let chunks = chunker()
            .chunk(&chars_cfg(max, 0), &doc, &tk)
            .await
            .unwrap();
        for c in &chunks {
            // Chunks are byte-bounded; with ASCII content `len()` equals char count.
            assert!(
                c.text.len() <= max as usize + 10,
                "chunk {} exceeds budget: len={}",
                c.chunk_id,
                c.text.len()
            );
        }
    }

    #[tokio::test]
    async fn overlap_adds_trailing_context() {
        let tk = MockTokenizer::new();
        let body = "Sentence one. ".repeat(80);
        let doc = to_doc(&body);
        let no_overlap = chunker()
            .chunk(&chars_cfg(200, 0), &doc, &tk)
            .await
            .unwrap();
        let with_overlap = chunker()
            .chunk(&chars_cfg(200, 50), &doc, &tk)
            .await
            .unwrap();
        let total_no = no_overlap.iter().map(|c| c.text.len()).sum::<usize>();
        let total_with = with_overlap.iter().map(|c| c.text.len()).sum::<usize>();
        assert!(
            total_with > total_no,
            "expected overlap to add bytes; got with={total_with} no={total_no}"
        );
    }

    #[tokio::test]
    async fn does_not_split_mid_word() {
        let tk = MockTokenizer::new();
        let word = "supercalifragilisticexpialidocious ";
        let body = word.repeat(40);
        let doc = to_doc(&body);
        let chunks = chunker().chunk(&chars_cfg(80, 0), &doc, &tk).await.unwrap();
        // Every chunk should end on or after a whitespace — i.e. no chunk text
        // should end inside `word` without a trailing space-or-whitespace nearby.
        for c in &chunks {
            let last_char = c.text.chars().last().unwrap();
            assert!(
                last_char.is_whitespace()
                    || last_char.is_ascii_punctuation()
                    || c.text.ends_with(word.trim()),
                "chunk {} ends mid-word: '{}'",
                c.chunk_id,
                c.text
            );
        }
    }
}
