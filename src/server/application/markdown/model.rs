#[derive(Debug, Clone)]
pub struct Document {
    pub source: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub text: String,
    pub span: Span,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub char_start: usize,
    pub char_end: usize,
}

#[derive(Debug, Clone)]
pub enum BlockKind {
    Heading(HeadingBlock),
    Paragraph,
    List,
    CodeFence,
    BlockQuote,
    Table,
    Html,
    ThematicBreak,
    Other,
}

#[derive(Debug, Clone)]
pub struct HeadingBlock {
    pub depth: u8,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct SectionBlock {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub heading: String,
}

#[derive(Debug, Clone)]
pub struct SegmentBlock {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub heading: String,
    pub atomic: bool,
}

#[derive(Debug, Clone)]
pub struct TextUnit {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub atomic: bool,
}

impl Block {
    pub fn heading(&self) -> Option<&HeadingBlock> {
        match &self.kind {
            BlockKind::Heading(heading) => Some(heading),
            _ => None,
        }
    }

    pub fn is_atomic_segment(&self) -> bool {
        matches!(
            self.kind,
            BlockKind::List
                | BlockKind::CodeFence
                | BlockKind::BlockQuote
                | BlockKind::Table
                | BlockKind::Html
                | BlockKind::ThematicBreak
                | BlockKind::Other
        )
    }

    pub fn is_atomic_text_unit(&self) -> bool {
        !matches!(self.kind, BlockKind::Paragraph)
    }
}
