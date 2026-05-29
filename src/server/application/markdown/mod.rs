mod dto;
mod heading_path;
mod model;
mod sections;
mod segments;
mod text_units;

pub use dto::{block_to_dto, document_to_dto_blocks};
pub use model::{
    Block, BlockKind, Document, HeadingBlock, SectionBlock, SegmentBlock, Span, TextUnit,
};
