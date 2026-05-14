use markdown::mdast::Node;

use crate::server::application::markdown::{Block, BlockKind, Document, HeadingBlock, Span};
use crate::server::application::AppError;

pub struct MdastMapper;

impl MdastMapper {
    pub fn map_document(source: &str, root: Node) -> Result<Document, AppError> {
        let children = match root {
            Node::Root(root) => root.children,
            _ => {
                return Err(AppError::Internal(
                    "markdown parser returned non-root document".into(),
                ));
            }
        };

        let mut blocks = Vec::new();
        let mut raw_blocks = Vec::new();
        for child in children {
            if let Some(block) = map_block(&child)? {
                raw_blocks.push(block);
            }
        }

        if raw_blocks.is_empty() {
            if source.is_empty() {
                return Ok(Document {
                    source: String::new(),
                    blocks: Vec::new(),
                });
            }
            return Ok(Document {
                source: source.to_string(),
                blocks: vec![Block {
                    text: source.to_string(),
                    span: Span {
                        char_start: 0,
                        char_end: source.chars().count(),
                    },
                    kind: BlockKind::Other,
                }],
            });
        }

        if raw_blocks[0].start > 0 {
            blocks.push(make_block(
                source,
                0,
                raw_blocks[0].start,
                BlockKind::Other,
            )?);
        }

        for (idx, raw) in raw_blocks.iter().enumerate() {
            let end = raw_blocks
                .get(idx + 1)
                .map(|next| next.start)
                .unwrap_or(source.len())
                .max(raw.end);
            blocks.push(make_block(source, raw.start, end, raw.kind.clone())?);
        }

        Ok(Document {
            source: source.to_string(),
            blocks,
        })
    }
}

#[derive(Debug, Clone)]
struct RawBlock {
    start: usize,
    end: usize,
    kind: BlockKind,
}

fn map_block(node: &Node) -> Result<Option<RawBlock>, AppError> {
    let Some((start, end)) = node_offsets(node) else {
        return Ok(None);
    };
    if start >= end {
        return Ok(None);
    }

    let kind = match node {
        Node::Heading(heading) => BlockKind::Heading(HeadingBlock {
            depth: heading.depth,
            text: plain_text(node),
        }),
        Node::Paragraph(_) => BlockKind::Paragraph,
        Node::List(_) => BlockKind::List,
        Node::Code(_) => BlockKind::CodeFence,
        Node::Blockquote(_) => BlockKind::BlockQuote,
        Node::Table(_) => BlockKind::Table,
        Node::Html(_) => BlockKind::Html,
        Node::ThematicBreak(_) => BlockKind::ThematicBreak,
        _ => BlockKind::Other,
    };

    Ok(Some(RawBlock { start, end, kind }))
}

fn make_block(source: &str, start: usize, end: usize, kind: BlockKind) -> Result<Block, AppError> {
    let text = source
        .get(start..end)
        .ok_or_else(|| AppError::Internal(format!("invalid markdown span {start}..{end}")))?
        .to_string();
    Ok(Block {
        span: Span {
            char_start: source[..start].chars().count(),
            char_end: source[..end].chars().count(),
        },
        text,
        kind,
    })
}

fn node_offsets(node: &Node) -> Option<(usize, usize)> {
    let position = node.position()?;
    Some((position.start.offset, position.end.offset))
}

fn plain_text(node: &Node) -> String {
    let mut out = String::new();
    append_plain_text(node, &mut out);
    out.trim().to_owned()
}

fn append_plain_text(node: &Node, out: &mut String) {
    match node {
        Node::Text(text) => out.push_str(&text.value),
        Node::InlineCode(code) => out.push_str(&code.value),
        Node::Code(code) => out.push_str(&code.value),
        Node::Break(_) => out.push(' '),
        _ => {
            if let Some(children) = node.children() {
                for child in children {
                    append_plain_text(child, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use markdown::{to_mdast, ParseOptions};

    #[test]
    fn maps_heading_text_and_preserves_offsets() {
        let source = "# Heading\n\nParagraph.\n";
        let root = to_mdast(source, &ParseOptions::gfm()).unwrap();

        let doc = MdastMapper::map_document(source, root).unwrap();

        assert_eq!(doc.blocks.len(), 2);
        match &doc.blocks[0].kind {
            BlockKind::Heading(heading) => assert_eq!(heading.text, "Heading"),
            other => panic!("expected heading block, got {other:?}"),
        }
        assert_eq!(doc.blocks[0].span.char_start, 0);
        assert_eq!(doc.blocks[0].span.char_end, "# Heading\n\n".chars().count());
        assert_eq!(doc.blocks[1].text, "Paragraph.\n");
    }
}
