use enum_map::Enum;

/// MDAST node types we care about for chunk-boundary penalties, plus `Word`
/// and `Sentence` which are synthesized by `MdParser` rather than coming from
/// the mdast tree.
#[derive(Debug, Copy, Clone, Enum, PartialEq, Eq, Hash)]
pub enum NodeType {
    Root,
    Blockquote,
    FootnoteDefinition,
    MdxJsxFlowElement,
    List,
    MdxjsEsm,
    Toml,
    Yaml,
    Break,
    InlineCode,
    InlineMath,
    Delete,
    Emphasis,
    MdxTextExpression,
    FootnoteReference,
    Html,
    Image,
    ImageReference,
    MdxJsxTextElement,
    Link,
    LinkReference,
    Strong,
    Text,
    Code,
    Math,
    MdxFlowExpression,
    Heading,
    Table,
    ThematicBreak,
    TableRow,
    TableCell,
    ListItem,
    Definition,
    Paragraph,
    Word,
    Sentence,
}
