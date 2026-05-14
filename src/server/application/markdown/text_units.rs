use super::{BlockKind, Document, TextUnit};

impl Document {
    pub fn llm_text_units(&self) -> Vec<TextUnit> {
        let mut units = Vec::new();
        let mut idx = 0usize;

        while idx < self.blocks.len() {
            let block = &self.blocks[idx];
            if matches!(block.kind, BlockKind::Paragraph)
                && self
                    .blocks
                    .get(idx + 1)
                    .map(|next| matches!(next.kind, BlockKind::List))
                    .unwrap_or(false)
                && block.text.trim_end().ends_with(':')
            {
                let next = &self.blocks[idx + 1];
                units.push(TextUnit {
                    text: format!("{}{}", block.text, next.text),
                    char_start: block.span.char_start,
                    char_end: next.span.char_end,
                    atomic: true,
                });
                idx += 2;
                continue;
            }

            if block.is_atomic_text_unit() {
                units.push(TextUnit {
                    text: block.text.clone(),
                    char_start: block.span.char_start,
                    char_end: block.span.char_end,
                    atomic: true,
                });
            } else {
                units.extend(split_prose_on_terminators(
                    &block.text,
                    block.span.char_start,
                ));
            }

            idx += 1;
        }

        bind_headings_to_following_units(units)
    }
}

fn bind_headings_to_following_units(units: Vec<TextUnit>) -> Vec<TextUnit> {
    let mut out = Vec::with_capacity(units.len());
    let mut pending_heading = String::new();
    let mut pending_start = 0usize;
    let mut pending_end = 0usize;

    for unit in units {
        if is_heading_unit(&unit) {
            if pending_heading.is_empty() {
                pending_start = unit.char_start;
            }
            pending_end = unit.char_end;
            pending_heading.push_str(&unit.text);
            continue;
        }

        if pending_heading.is_empty() {
            out.push(unit);
            continue;
        }

        let mut text = std::mem::take(&mut pending_heading);
        text.push_str(&unit.text);
        out.push(TextUnit {
            text,
            char_start: pending_start,
            char_end: unit.char_end,
            atomic: true,
        });
    }

    if !pending_heading.trim().is_empty() {
        out.push(TextUnit {
            text: pending_heading,
            char_start: pending_start,
            char_end: pending_end,
            atomic: true,
        });
    }

    out
}

fn is_heading_unit(unit: &TextUnit) -> bool {
    let mut non_blank_lines = unit.text.lines().filter(|line| !line.trim().is_empty());
    let Some(line) = non_blank_lines.next() else {
        return false;
    };
    non_blank_lines.next().is_none() && parse_atx_heading(line).is_some()
}

fn split_prose_on_terminators(text: &str, char_start: usize) -> Vec<TextUnit> {
    let chars: Vec<char> = text.chars().collect();
    let mut units = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        if is_terminating_punctuation(chars[i])
            && chars.get(i + 1).map(|c| c.is_whitespace()).unwrap_or(true)
        {
            let mut end = i + 1;
            while chars.get(end).map(|c| c.is_whitespace()).unwrap_or(false) {
                end += 1;
            }
            push_unit_from_chars(&mut units, &chars, start, end, char_start, false);
            start = end;
            i = end;
            continue;
        }
        i += 1;
    }

    if start < chars.len() {
        push_unit_from_chars(&mut units, &chars, start, chars.len(), char_start, false);
    }

    units
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
    if text.trim().is_empty() {
        return;
    }
    units.push(TextUnit {
        text,
        char_start: base_char_start + start,
        char_end: base_char_start + end,
        atomic,
    });
}

fn is_terminating_punctuation(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | ';' | ':' | '。' | '！' | '？')
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
    use crate::server::application::ports::MarkdownParser;
    use crate::server::infrastructure::markdown::MarkdownRsParser;

    #[test]
    fn llm_text_units_bind_heading_and_colon_intro_list() {
        let parser = MarkdownRsParser;
        let doc = parser
            .parse("## Heading\nIntro:\n- first\n- second\n\nTail sentence.")
            .unwrap();

        let units = doc.llm_text_units();

        assert_eq!(units.len(), 2);
        assert!(units[0].atomic);
        assert!(units[0].text.starts_with("## Heading\nIntro:\n- first"));
        assert!(units[0].text.contains("- second"));
        assert_eq!(units[1].text, "Tail sentence.");
    }
}
