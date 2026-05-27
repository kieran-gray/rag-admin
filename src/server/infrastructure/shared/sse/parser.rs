use std::str;

use async_stream::stream;
use bytes::Bytes;
use futures_util::stream::{BoxStream, StreamExt};

use crate::server::application::AppError;

pub type ByteStream = BoxStream<'static, Result<Bytes, reqwest::Error>>;
pub type DataStream = BoxStream<'static, Result<String, AppError>>;

pub fn sse_data_stream(mut bytes: ByteStream) -> DataStream {
    let s = stream! {
        let mut buffer = String::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    yield Err(AppError::Upstream(format!("sse byte read: {e}")));
                    return;
                }
            };
            let text = match str::from_utf8(&chunk) {
                Ok(s) => s.to_string(),
                Err(e) => {
                    yield Err(AppError::Upstream(format!("sse utf8: {e}")));
                    return;
                }
            };
            normalise_into(&text, &mut buffer);
            while let Some(idx) = buffer.find("\n\n") {
                let mut block: String = buffer.drain(..idx + 2).collect();
                block.truncate(block.len() - 2);
                if let Some(data) = extract_data(&block) {
                    yield Ok(data);
                }
            }
        }
        if !buffer.trim().is_empty() {
            if let Some(data) = extract_data(buffer.trim()) {
                yield Ok(data);
            }
        }
    };
    s.boxed()
}

fn normalise_into(src: &str, dest: &mut String) {
    for ch in src.chars() {
        if ch == '\r' {
            dest.push('\n');
        } else {
            dest.push(ch);
        }
    }
}

fn extract_data(block: &str) -> Option<String> {
    let mut data_lines: Vec<&str> = Vec::new();
    for line in block.split('\n') {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_data_joins_multiline_payloads() {
        let block = "event: meta\ndata: line1\ndata: line2";
        assert_eq!(extract_data(block).as_deref(), Some("line1\nline2"));
    }

    #[test]
    fn extract_data_strips_single_leading_space() {
        let block = "data: hello";
        assert_eq!(extract_data(block).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_data_preserves_internal_spaces() {
        let block = "data:  doubled";
        assert_eq!(extract_data(block).as_deref(), Some(" doubled"));
    }

    #[test]
    fn extract_data_skips_blocks_with_no_data_lines() {
        assert_eq!(extract_data(": comment\nevent: foo"), None);
    }
}
