use std::sync::Arc;

use async_trait::async_trait;

use crate::server::application::chunking::{ChunkOutput, DocumentChunker, TokenBudget};
use crate::server::application::markdown::{Document, TextUnit};
use crate::server::application::ports::{ChatClient, ChatRequest, ChatResponseFormat, Tokenizer};
use crate::server::application::AppError;
use crate::server::domain::configuration::generation_model::GenerationModelRepository;
use crate::shared::{ChunkStrategy, ChunkingConfig};

const SYSTEM_PROMPT: &str = "You split blog text into compact, self-contained retrieval chunks. \
    The text has been divided into numbered micro-chunks marked with <|start_chunk_X|> and \
    <|end_chunk_X|> tags, where X is the chunk number. Choose split points so each final chunk is \
    narrow enough to avoid irrelevant context, but complete enough that a retriever selecting only \
    a few chunks can still recover the full evidence needed to answer detailed questions. Prefer \
    compact evidence packages over broad thematic sections or isolated fragments. Split when the \
    next micro-chunk starts a clearly independent claim, component, API, file path, responsibility, \
    trade-off, example, or conclusion. Keep adjacent micro-chunks together when they form one \
    answerable unit: a heading and its explanation, an intro sentence and its list, a sequence of \
    steps, a code block and its explanation, or list items that are likely to be asked about \
    together. Do not split so aggressively that a multi-part answer would require many tiny chunks. \
    Respond only with the IDs of micro-chunks after which a split should occur. THE CHUNK IDs MUST \
    BE IN ASCENDING ORDER. Your response should be in the form: 'split_after: 3, 5'.";

const WINDOW_TOKENS: usize = 1024;

#[derive(Debug, Clone)]
struct MicroChunk {
    text: String,
    char_start: usize,
    char_end: usize,
}

pub struct LlmChunker {
    chat_client: Arc<dyn ChatClient>,
    generation_models: Arc<dyn GenerationModelRepository>,
}

impl LlmChunker {
    pub fn create(
        chat_client: Arc<dyn ChatClient>,
        generation_models: Arc<dyn GenerationModelRepository>,
    ) -> Self {
        Self {
            chat_client,
            generation_models,
        }
    }
}

#[async_trait]
impl DocumentChunker for LlmChunker {
    fn strategy(&self) -> ChunkStrategy {
        ChunkStrategy::Llm
    }

    async fn chunk(
        &self,
        config: &ChunkingConfig,
        source: &Document,
        tokenizer: &dyn Tokenizer,
    ) -> Result<Vec<ChunkOutput>, AppError> {
        let ChunkingConfig::Llm(config) = config else {
            unreachable!()
        };

        let target_tokens = config.target_tokens.max(1) as usize;
        let micro_size = config.micro_chunk_tokens.max(32) as usize;
        let budget = TokenBudget::new(tokenizer);
        let units = source.llm_text_units();
        let micro_chunks = split_into_micro_chunks(units, micro_size, &budget)?;

        if micro_chunks.is_empty() {
            return Ok(Vec::new());
        }

        let model_name = self
            .resolve_generation_model(config.generation_model_id)
            .await?;

        let split_points = find_split_points(
            &micro_chunks,
            self.chat_client.as_ref(),
            &model_name,
            &budget,
        )
        .await?;
        let merged = merge_micro_chunks(&micro_chunks, &split_points, target_tokens, &budget)?;

        Ok(merged
            .into_iter()
            .enumerate()
            .map(|(i, mc)| chunk_output_from_micro_chunk(i, mc))
            .collect())
    }
}

impl LlmChunker {
    async fn resolve_generation_model(&self, model_id: uuid::Uuid) -> Result<String, AppError> {
        self.generation_models
            .find_by_id(model_id)
            .await?
            .map(|m| m.model)
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "LLM chunking: generation model {model_id} not found in registry"
                ))
            })
    }
}

fn chunk_output_from_micro_chunk(chunk_id: usize, mc: MicroChunk) -> ChunkOutput {
    let (text, char_start, char_end) = trim_chunk_text(&mc.text, mc.char_start, mc.char_end);
    let heading = extract_heading(&text);
    ChunkOutput {
        chunk_id: chunk_id as u32,
        heading,
        text,
        char_start: char_start as u32,
        char_end: char_end as u32,
    }
}

fn trim_chunk_text(text: &str, char_start: usize, char_end: usize) -> (String, usize, usize) {
    let text_len = text.chars().count();
    let leading = text.chars().take_while(|c| c.is_whitespace()).count();
    let trailing = text.chars().rev().take_while(|c| c.is_whitespace()).count();

    if leading + trailing >= text_len {
        return (String::new(), char_start, char_start);
    }

    let trimmed: String = text
        .chars()
        .skip(leading)
        .take(text_len - leading - trailing)
        .collect();

    (
        trimmed,
        char_start + leading,
        char_end.saturating_sub(trailing),
    )
}

fn split_into_micro_chunks(
    units: Vec<TextUnit>,
    target_tokens: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<MicroChunk>, AppError> {
    let target_tokens = target_tokens.max(1);
    pack_text_units(units, target_tokens, budget)
}

fn push_unit_from_chars(
    units: &mut Vec<TextUnit>,
    chars: &[char],
    start: usize,
    end: usize,
    base_char_start: usize,
    atomic: bool,
) {
    let text: String = chars[start..end].iter().collect();
    push_unit(
        units,
        text,
        base_char_start + start,
        base_char_start + end,
        atomic,
    );
}

fn push_unit(
    units: &mut Vec<TextUnit>,
    text: String,
    char_start: usize,
    char_end: usize,
    atomic: bool,
) {
    if text.trim().is_empty() {
        return;
    }
    units.push(TextUnit {
        text,
        char_start,
        char_end,
        atomic,
    });
}

fn pack_text_units(
    units: Vec<TextUnit>,
    target_tokens: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<MicroChunk>, AppError> {
    let mut out = Vec::new();
    let mut current_text = String::new();
    let mut current_start = 0usize;
    let mut current_end = 0usize;

    for unit in units {
        if unit.atomic {
            flush_micro_chunk(&mut out, &mut current_text, current_start, current_end);
            for piece in split_oversized_unit(unit, target_tokens, budget)? {
                out.push(MicroChunk {
                    text: piece.text,
                    char_start: piece.char_start,
                    char_end: piece.char_end,
                });
            }
            continue;
        }

        let pieces = split_oversized_unit(unit, target_tokens, budget)?;
        for piece in pieces {
            let merged = format!("{}{}", current_text, piece.text);
            if !current_text.is_empty() && budget.count_str(&merged)? > target_tokens {
                flush_micro_chunk(&mut out, &mut current_text, current_start, current_end);
            }

            if current_text.is_empty() {
                current_start = piece.char_start;
            }
            current_end = piece.char_end;
            current_text.push_str(&piece.text);
        }
    }

    flush_micro_chunk(&mut out, &mut current_text, current_start, current_end);
    Ok(out)
}

fn flush_micro_chunk(
    out: &mut Vec<MicroChunk>,
    current_text: &mut String,
    current_start: usize,
    current_end: usize,
) {
    if current_text.trim().is_empty() {
        current_text.clear();
        return;
    }
    out.push(MicroChunk {
        text: std::mem::take(current_text),
        char_start: current_start,
        char_end: current_end,
    });
}

fn split_oversized_unit(
    unit: TextUnit,
    target_tokens: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<TextUnit>, AppError> {
    if budget.count_str(&unit.text)? <= target_tokens {
        return Ok(vec![unit]);
    }

    let chars: Vec<char> = unit.text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let prefix_len = budget.max_prefix_chars(&chars[start..], target_tokens)?;
        let hard_end = (start + prefix_len).min(chars.len());
        let end = if hard_end < chars.len() {
            last_whitespace_after_half(&chars[start..hard_end])
                .map(|offset| start + offset + 1)
                .unwrap_or(hard_end)
        } else {
            hard_end
        };
        push_unit_from_chars(&mut out, &chars, start, end, unit.char_start, false);
        start = end;
    }

    Ok(out)
}

fn last_whitespace_after_half(chars: &[char]) -> Option<usize> {
    let half = chars.len() / 2;
    chars
        .iter()
        .enumerate()
        .rev()
        .find(|(i, c)| *i >= half && c.is_whitespace())
        .map(|(i, _)| i)
}

async fn find_split_points(
    micro_chunks: &[MicroChunk],
    client: &dyn ChatClient,
    model: &str,
    budget: &TokenBudget<'_>,
) -> Result<Vec<usize>, AppError> {
    let n = micro_chunks.len();
    if n <= 1 {
        return Ok(vec![n.saturating_sub(1)]);
    }

    let mut split_after: Vec<usize> = Vec::new();
    let mut current = 0usize;

    while current < n.saturating_sub(3) {
        let mut window = String::new();

        for (i, item) in micro_chunks.iter().enumerate().take(n).skip(current) {
            let tagged = format!(
                "<|start_chunk_{}|>{}<|end_chunk_{}|>",
                i + 1,
                item.text,
                i + 1
            );
            let candidate = format!("{window}{tagged}");
            if !window.is_empty() && budget.count_str(&candidate)? > WINDOW_TOKENS {
                break;
            }
            window = candidate;
        }

        let user_msg = format!(
            "CHUNKED_TEXT: {window}\n\nRespond only with the IDs of chunks where a split should occur. \
            You MUST respond with at least one split. IDs must be >= {}.",
            current + 1
        );

        let response = client
            .chat(ChatRequest {
                model: model.to_string(),
                system: SYSTEM_PROMPT.to_string(),
                user: user_msg,
                temperature: 0.2,
                response_format: ChatResponseFormat::Text,
            })
            .await?;
        let numbers = parse_split_response(&response.content, current + 1);

        if numbers.is_empty() {
            // No valid split found; advance one step to avoid infinite loop
            current += 1;
            continue;
        }

        for &n_id in &numbers {
            let idx = n_id - 1; // convert 1-indexed to 0-indexed
            if idx < n && !split_after.contains(&idx) {
                split_after.push(idx);
            }
        }

        current = *numbers.last().unwrap();
    }

    // Ensure the last chunk is always included
    if split_after.last() != Some(&(n - 1)) {
        split_after.push(n - 1);
    }

    split_after.sort_unstable();
    split_after.dedup();
    Ok(split_after)
}

fn parse_split_response(response: &str, min_id: usize) -> Vec<usize> {
    let line = response
        .lines()
        .find(|l| l.contains("split_after:"))
        .unwrap_or(response);

    let numbers: Vec<usize> = line
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= min_id)
        .collect();

    // Verify ascending order
    let is_ascending = numbers.windows(2).all(|w| w[0] < w[1]);
    if !is_ascending {
        return Vec::new();
    }

    numbers
}

fn merge_micro_chunks(
    micro_chunks: &[MicroChunk],
    split_points: &[usize],
    max_tokens: usize,
    budget: &TokenBudget<'_>,
) -> Result<Vec<MicroChunk>, AppError> {
    let mut out = Vec::new();
    let mut current_text = String::new();
    let mut current_start = micro_chunks.first().map(|m| m.char_start).unwrap_or(0);
    let mut last_end = 0usize;

    for (i, mc) in micro_chunks.iter().enumerate() {
        let candidate = format!("{}{}", current_text, mc.text);
        if !current_text.is_empty() && budget.count_str(&candidate)? > max_tokens {
            out.push(MicroChunk {
                text: current_text.clone(),
                char_start: current_start,
                char_end: last_end,
            });
            current_text.clear();
            current_start = mc.char_start;
        }

        current_text.push_str(&mc.text);
        last_end = mc.char_end;

        if split_points.contains(&i) {
            out.push(MicroChunk {
                text: current_text.clone(),
                char_start: current_start,
                char_end: last_end,
            });
            current_text.clear();
            if i + 1 < micro_chunks.len() {
                current_start = micro_chunks[i + 1].char_start;
            }
        }
    }

    if !current_text.trim().is_empty() {
        out.push(MicroChunk {
            text: current_text,
            char_start: current_start,
            char_end: last_end,
        });
    }

    Ok(out)
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
    while depth < bytes.len() && bytes[depth] == b'#' {
        depth += 1;
    }
    if depth == 0 || depth > 6 {
        return None;
    }
    let after = &line[depth..];
    let first = after.chars().next()?;
    if !first.is_whitespace() {
        return None;
    }
    let text = after.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some((depth, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::application::ports::MarkdownParser;
    use crate::server::application::test_support::MockTokenizer;
    use crate::server::infrastructure::markdown::MarkdownRsParser;

    fn tokenizer() -> MockTokenizer {
        MockTokenizer::new()
    }

    fn joined_text(chunks: &[MicroChunk]) -> String {
        chunks.iter().map(|c| c.text.as_str()).collect()
    }

    fn split_test(body: &str, target_tokens: usize) -> Vec<MicroChunk> {
        let tokenizer = tokenizer();
        let budget = TokenBudget::new(&tokenizer);
        let units = MarkdownRsParser.parse(body).unwrap().llm_text_units();
        split_into_micro_chunks(units, target_tokens, &budget).unwrap()
    }

    fn slice_chars(text: &str, start: usize, end: usize) -> String {
        text.chars().skip(start).take(end - start).collect()
    }

    #[test]
    fn split_into_micro_chunks_basic() {
        let body = "paragraph one.\n\nparagraph two.\n\nparagraph three.";
        let chunks = split_test(body, 4);
        assert!(chunks.len() >= 2);
        let full = joined_text(&chunks);
        assert!(full.contains("paragraph one"));
        assert!(full.contains("paragraph three"));
    }

    #[test]
    fn split_into_micro_chunks_uses_terminating_punctuation_boundaries() {
        let body = "Alpha sentence. Beta clause; Gamma question? Delta exclaim!";
        let chunks = split_test(body, 2);

        assert_eq!(joined_text(&chunks), body);
        assert_eq!(
            chunks.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            vec![
                "Alpha sentence. ",
                "Beta clause; ",
                "Gamma question? ",
                "Delta exclaim!"
            ]
        );
        for chunk in chunks {
            assert_eq!(
                slice_chars(body, chunk.char_start, chunk.char_end),
                chunk.text
            );
        }
    }

    #[test]
    fn split_into_micro_chunks_binds_headings_to_following_chunk() {
        let body = "Intro sentence.\n## Heading\nBody sentence.";
        let chunks = split_test(body, 1000);
        let heading_start = "Intro sentence.\n".chars().count();

        assert_eq!(joined_text(&chunks), body);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].text, "## Heading\nBody sentence.");
        assert_eq!(chunks[1].char_start, heading_start);
        assert_eq!(
            slice_chars(body, chunks[1].char_start, chunks[1].char_end),
            chunks[1].text
        );
    }

    #[test]
    fn split_into_micro_chunks_keeps_fenced_code_blocks_atomic() {
        let fence = "```rust\nfn main() { println!(\"a.b\"); }\n```\n";
        let body = format!("Intro sentence.\n{fence}After.");
        let chunks = split_test(&body, 20);

        assert_eq!(joined_text(&chunks), body);
        let fence_chunks = chunks
            .iter()
            .filter(|chunk| chunk.text.contains("```rust"))
            .collect::<Vec<_>>();
        assert_eq!(fence_chunks.len(), 1);
        assert_eq!(fence_chunks[0].text, fence);
    }

    #[test]
    fn split_into_micro_chunks_keeps_contiguous_list_items_together() {
        let body = "- first item.\n- second item.\nclosing sentence.";
        let chunks = split_test(body, 1000);

        assert_eq!(joined_text(&chunks), body);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, body);
    }

    #[test]
    fn split_into_micro_chunks_binds_colon_intro_to_numbered_list() {
        let body = concat!(
            "Durable Objects give you:\n\n",
            "1. A single-threaded execution context per aggregate ID.\n",
            "2. In-process, synchronous SQLite. Projections catch up in milliseconds.\n",
            "3. Alarms, as the do-something-about-events mechanism.\n\n",
            "There are catches:\n\n",
            "- Storage per DO is capped.\n",
            "- Cross-aggregate queries need D1.\n"
        );
        let chunks = split_test(body, 150);

        assert_eq!(joined_text(&chunks), body);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.starts_with("Durable Objects give you:"));
        assert!(chunks[0].text.contains("1. A single-threaded"));
        assert!(chunks[0].text.contains("3. Alarms"));
        assert!(chunks[1].text.starts_with("There are catches:"));
        assert!(chunks[1].text.contains("- Storage per DO"));
        for chunk in chunks {
            assert_eq!(
                slice_chars(body, chunk.char_start, chunk.char_end),
                chunk.text
            );
        }
    }

    #[test]
    fn split_into_micro_chunks_keeps_wrapped_list_item_with_item() {
        let body = "- first item starts here\n  and continues here.\n- second item.\n";
        let chunks = split_test(body, 1000);

        assert_eq!(joined_text(&chunks), body);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, body);
    }

    #[test]
    fn split_into_micro_chunks_splits_oversized_prose_without_punctuation() {
        let body = "alpha beta gamma delta epsilon zeta eta theta";
        let chunks = split_test(body, 4);

        assert!(chunks.len() > 1);
        assert_eq!(joined_text(&chunks), body);
        assert!(chunks.iter().all(|chunk| !chunk.text.trim().is_empty()));
    }

    #[test]
    fn chunk_output_trims_text_and_preserves_offsets() {
        let text = "\n  £ Trimmed text. \n";
        let mc = MicroChunk {
            text: text.to_string(),
            char_start: 7,
            char_end: 7 + text.chars().count(),
        };

        let output = chunk_output_from_micro_chunk(2, mc);

        assert_eq!(output.chunk_id, 2);
        assert_eq!(output.text, "£ Trimmed text.");
        assert_eq!(output.char_start, 10);
        assert_eq!(
            output.char_end,
            10 + "£ Trimmed text.".chars().count() as u32
        );
    }

    #[test]
    fn parse_split_response_basic() {
        let nums = parse_split_response("split_after: 2, 5, 8", 1);
        assert_eq!(nums, vec![2, 5, 8]);
    }

    #[test]
    fn parse_split_response_filters_below_min() {
        let nums = parse_split_response("split_after: 1, 2, 5", 3);
        assert_eq!(nums, vec![5]);
    }

    #[test]
    fn parse_split_response_rejects_non_ascending() {
        let nums = parse_split_response("split_after: 5, 3, 8", 1);
        assert!(nums.is_empty());
    }

    #[test]
    fn merge_micro_chunks_combines_correctly() {
        let micro = vec![
            MicroChunk {
                text: "A".into(),
                char_start: 0,
                char_end: 1,
            },
            MicroChunk {
                text: "B".into(),
                char_start: 1,
                char_end: 2,
            },
            MicroChunk {
                text: "C".into(),
                char_start: 2,
                char_end: 3,
            },
        ];
        // Split after index 1 (keep 0+1 together, 2 alone)
        let tokenizer = tokenizer();
        let budget = TokenBudget::new(&tokenizer);
        let merged = merge_micro_chunks(&micro, &[1, 2], 10, &budget).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "AB");
        assert_eq!(merged[1].text, "C");
        assert_eq!(merged[0].char_start, 0);
        assert_eq!(merged[0].char_end, 2);
    }

    #[test]
    fn extract_heading_finds_first_heading() {
        let text = "some intro\n\n## My Section\n\nbody text";
        assert_eq!(extract_heading(text), "My Section");
    }

    #[test]
    fn extract_heading_returns_empty_when_none() {
        let text = "just plain text with no heading";
        assert_eq!(extract_heading(text), "");
    }
}
