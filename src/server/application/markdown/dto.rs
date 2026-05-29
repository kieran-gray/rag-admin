use crate::shared::contracts::{MarkdownBlockDto, MarkdownBlockKindDto};

use super::{Block, BlockKind, Document};

pub fn block_to_dto(block: &Block) -> MarkdownBlockDto {
    let (kind, heading_depth) = match &block.kind {
        BlockKind::Heading(h) => (MarkdownBlockKindDto::Heading, Some(h.depth)),
        BlockKind::Paragraph => (MarkdownBlockKindDto::Paragraph, None),
        BlockKind::List => (MarkdownBlockKindDto::List, None),
        BlockKind::CodeFence => (MarkdownBlockKindDto::CodeFence, None),
        BlockKind::BlockQuote => (MarkdownBlockKindDto::BlockQuote, None),
        BlockKind::Table => (MarkdownBlockKindDto::Table, None),
        BlockKind::Html => (MarkdownBlockKindDto::Html, None),
        BlockKind::ThematicBreak => (MarkdownBlockKindDto::ThematicBreak, None),
        BlockKind::Other => (MarkdownBlockKindDto::Other, None),
    };
    MarkdownBlockDto {
        kind,
        html: markdown::to_html(&block.text),
        char_start: block.span.char_start as u32,
        char_end: block.span.char_end as u32,
        heading_depth,
    }
}

pub fn document_to_dto_blocks(document: &Document) -> Vec<MarkdownBlockDto> {
    document.blocks.iter().map(block_to_dto).collect()
}
