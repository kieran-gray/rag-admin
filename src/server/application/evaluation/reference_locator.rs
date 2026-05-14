use crate::server::application::AppError;
use crate::shared::{EvaluationQuestionDto, EvaluationReferenceDto};

use super::ports::GeneratedEvaluationQuestion;

const MAX_REFERENCE_COUNT: usize = 5;

pub struct ReferenceLocator;

impl ReferenceLocator {
    pub fn generated_to_question(
        generated: &GeneratedEvaluationQuestion,
        document: &str,
    ) -> Result<EvaluationQuestionDto, AppError> {
        if generated.references.len() > MAX_REFERENCE_COUNT {
            return Err(AppError::Validation(format!(
                "too many references: {}",
                generated.references.len()
            )));
        }

        let mut references = Vec::with_capacity(generated.references.len());
        for reference in &generated.references {
            let (start, end) = Self::find_reference(document, reference).ok_or_else(|| {
                AppError::Validation(format!(
                    "reference was not found in document: {}",
                    truncate(reference, 120)
                ))
            })?;
            references.push(EvaluationReferenceDto {
                content: document[start..end].to_string(),
                char_start: byte_to_char_index(document, start) as u32,
                char_end: byte_to_char_index(document, end) as u32,
                embedding: None,
            });
        }

        Ok(EvaluationQuestionDto {
            question: generated.question.clone(),
            references,
            embedding: None,
        })
    }

    pub fn find_reference(document: &str, reference: &str) -> Option<(usize, usize)> {
        if let Some(start) = document.find(reference) {
            return Some((start, start + reference.len()));
        }
        find_despite_whitespace(document, reference)
            .or_else(|| find_despite_markdown_formatting(document, reference))
            .or_else(|| find_by_token_alignment(document, reference))
    }
}

fn find_despite_whitespace(document: &str, reference: &str) -> Option<(usize, usize)> {
    let words: Vec<&str> = reference.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    for (start, _) in document.char_indices() {
        let mut pos = start;
        let mut matched = true;
        for (i, word) in words.iter().enumerate() {
            if i > 0 {
                pos = skip_whitespace(document, pos);
            }
            if pos > document.len() || !document[pos..].starts_with(word) {
                matched = false;
                break;
            }
            pos += word.len();
        }
        if matched {
            return Some((start, pos));
        }
    }
    None
}

fn skip_whitespace(document: &str, mut pos: usize) -> usize {
    while pos < document.len() {
        let Some(ch) = document[pos..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn find_despite_markdown_formatting(document: &str, reference: &str) -> Option<(usize, usize)> {
    let normalized_document = normalize_reference_text(document);
    let normalized_reference = normalize_reference_text(reference);
    let reference_text = normalized_reference.text.trim();

    if reference_text.is_empty() {
        return None;
    }

    let normalized_start = normalized_document.text.find(reference_text)?;
    let normalized_end = normalized_start + reference_text.len();
    let start = normalized_document
        .byte_to_original
        .get(normalized_start)
        .copied()?;
    let end = normalized_document
        .byte_to_original
        .get(normalized_end)
        .copied()
        .or_else(|| normalized_document.byte_to_original.last().copied())?;

    (start < end).then_some((start, end))
}

#[derive(Debug, Clone)]
struct AlignmentToken {
    text: String,
    start: usize,
    end: usize,
}

fn find_by_token_alignment(document: &str, reference: &str) -> Option<(usize, usize)> {
    let doc_tokens = tokenize_for_alignment(document);
    let ref_tokens = tokenize_for_alignment(reference);

    if doc_tokens.is_empty() || ref_tokens.is_empty() {
        return None;
    }

    find_contiguous_token_span(&doc_tokens, &ref_tokens)
        .or_else(|| find_ordered_token_span(&doc_tokens, &ref_tokens))
}

fn tokenize_for_alignment(value: &str) -> Vec<AlignmentToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut token_start = 0usize;
    let mut last_end = 0usize;

    for (byte, ch) in value.char_indices() {
        if let Some(normalized) = normalize_alignment_char(ch) {
            if current.is_empty() {
                token_start = byte;
            }
            current.push(normalized);
            last_end = byte + ch.len_utf8();
            continue;
        }

        if !current.is_empty() {
            tokens.push(AlignmentToken {
                text: std::mem::take(&mut current),
                start: token_start,
                end: last_end,
            });
        }
    }

    if !current.is_empty() {
        tokens.push(AlignmentToken {
            text: current,
            start: token_start,
            end: last_end,
        });
    }

    tokens
}

fn normalize_alignment_char(ch: char) -> Option<char> {
    let normalized = match ch {
        'A'..='Z' => ch.to_ascii_lowercase(),
        'a'..='z' | '0'..='9' => ch,
        '-' | '_' | '/' | '.' | ':' | '@' => ch,
        '\'' | '"' | '`' | '*' | '~' | '[' | ']' | '(' | ')' | '{' | '}' | '<' | '>' | '#' => {
            return None
        }
        _ if ch.is_ascii_punctuation() => return None,
        _ if ch.is_whitespace() => return None,
        _ => ch.to_ascii_lowercase(),
    };
    Some(normalized)
}

fn find_contiguous_token_span(
    document_tokens: &[AlignmentToken],
    reference_tokens: &[AlignmentToken],
) -> Option<(usize, usize)> {
    if reference_tokens.len() > document_tokens.len() {
        return None;
    }

    'outer: for start in 0..=document_tokens.len() - reference_tokens.len() {
        for (offset, reference) in reference_tokens.iter().enumerate() {
            if document_tokens[start + offset].text != reference.text {
                continue 'outer;
            }
        }

        let end_index = start + reference_tokens.len() - 1;
        return Some((document_tokens[start].start, document_tokens[end_index].end));
    }

    None
}

fn find_ordered_token_span(
    document_tokens: &[AlignmentToken],
    reference_tokens: &[AlignmentToken],
) -> Option<(usize, usize)> {
    let min_matches = reference_tokens.len().min(5);
    if min_matches < 2 {
        return None;
    }

    let mut matches = Vec::new();
    let mut doc_index = 0usize;

    for reference in reference_tokens {
        let mut found = None;
        while doc_index < document_tokens.len() {
            if document_tokens[doc_index].text == reference.text {
                found = Some(doc_index);
                doc_index += 1;
                break;
            }
            doc_index += 1;
        }

        if let Some(index) = found {
            matches.push(index);
        }
    }

    if matches.len() < min_matches {
        return None;
    }

    let coverage = matches.len() as f32 / reference_tokens.len() as f32;
    if coverage < 0.8 {
        return None;
    }

    let longest_run = longest_consecutive_run(&matches);
    if longest_run < 2 && matches.len() < reference_tokens.len() {
        return None;
    }

    let start_index = *matches.first()?;
    let end_index = *matches.last()?;
    let span_token_len = end_index.saturating_sub(start_index) + 1;
    let max_span_token_len = reference_tokens.len() * 2 + 8;
    if span_token_len > max_span_token_len {
        return None;
    }

    Some((
        document_tokens[start_index].start,
        document_tokens[end_index].end,
    ))
}

fn longest_consecutive_run(indexes: &[usize]) -> usize {
    let mut best = 0usize;
    let mut current = 0usize;
    let mut previous = None;

    for index in indexes {
        if previous.map(|prev| *index == prev + 1).unwrap_or(false) {
            current += 1;
        } else {
            current = 1;
        }
        best = best.max(current);
        previous = Some(*index);
    }

    best
}

struct NormalizedText {
    text: String,
    byte_to_original: Vec<usize>,
}

fn normalize_reference_text(value: &str) -> NormalizedText {
    let mut text = String::new();
    let mut byte_to_original = Vec::new();
    let mut last_was_space = false;

    for (original_byte, ch) in value.char_indices() {
        if ch.is_whitespace() {
            if !last_was_space {
                push_normalized_char(&mut text, &mut byte_to_original, ' ', original_byte);
                last_was_space = true;
            }
            continue;
        }

        let Some(normalized) = normalize_reference_char(ch) else {
            continue;
        };
        push_normalized_char(&mut text, &mut byte_to_original, normalized, original_byte);
        last_was_space = false;
    }

    byte_to_original.push(value.len());
    NormalizedText {
        text,
        byte_to_original,
    }
}

fn normalize_reference_char(ch: char) -> Option<char> {
    match ch {
        '"' | '\'' | '‘' | '’' | '“' | '”' => Some('"'),
        '`' => None,
        _ => Some(ch),
    }
}

fn push_normalized_char(
    text: &mut String,
    byte_to_original: &mut Vec<usize>,
    ch: char,
    original_byte: usize,
) {
    text.push(ch);
    for _ in 0..ch.len_utf8() {
        byte_to_original.push(original_byte);
    }
}

fn byte_to_char_index(document: &str, byte_index: usize) -> usize {
    document[..byte_index].chars().count()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_exact_reference() {
        let doc = "alpha beta gamma";
        assert_eq!(ReferenceLocator::find_reference(doc, "beta"), Some((6, 10)));
    }

    #[test]
    fn finds_reference_despite_whitespace() {
        let doc = "alpha beta\n\ngamma";
        assert_eq!(
            ReferenceLocator::find_reference(doc, "beta gamma"),
            Some((6, 17))
        );
    }

    #[test]
    fn finds_reference_despite_quote_formatting_in_list_intro() {
        let doc = concat!(
            "The handshake itself is the \"view event\". ",
            "When the `fetch` handler receives a WebSocket upgrade request, it:\n\n",
            "1. Ensures the in-memory state is initialized.\n",
            "2. Increments the in-memory total.\n"
        );
        let reference = concat!(
            "The handshake itself is the 'view event'. ",
            "When the fetch handler receives a WebSocket upgrade request, it: ",
            "1. Ensures the in-memory state is initialized."
        );

        let (start, end) = ReferenceLocator::find_reference(doc, reference).unwrap();

        assert_eq!(start, 0);
        assert!(doc[start..end].contains("\"view event\""));
        assert!(doc[start..end].contains("1. Ensures"));
    }

    #[test]
    fn finds_reference_despite_markdown_link_url_noise() {
        let doc = "Use [Workers AI](https://developers.cloudflare.com/workers-ai/) for inference.";
        let reference = "Use Workers AI for inference.";

        let (start, end) = ReferenceLocator::find_reference(doc, reference).unwrap();

        assert_eq!(
            &doc[start..end],
            "Use [Workers AI](https://developers.cloudflare.com/workers-ai/) for inference."
        );
    }

    #[test]
    fn finds_reference_when_model_drops_list_numbering() {
        let doc = concat!(
            "The handler does three things:\n",
            "1. Loads the aggregate.\n",
            "2. Validates the command.\n",
            "3. Persists the event.\n"
        );
        let reference =
            "The handler does three things: Loads the aggregate. Validates the command.";

        let (start, end) = ReferenceLocator::find_reference(doc, reference).unwrap();

        assert!(doc[start..end].contains("1. Loads the aggregate."));
        assert!(doc[start..end].contains("2. Validates the command."));
    }

    #[test]
    fn generated_question_uses_char_offsets() {
        let doc = "£ alpha beta";
        let generated = GeneratedEvaluationQuestion {
            question: "What word follows alpha?".into(),
            references: vec!["alpha beta".into()],
        };

        let question = ReferenceLocator::generated_to_question(&generated, doc).unwrap();

        assert_eq!(question.references[0].char_start, 2);
        assert_eq!(question.references[0].char_end, 12);
    }
}
