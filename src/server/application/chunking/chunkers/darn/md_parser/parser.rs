use markdown::mdast::Node;
use markdown::mdast::Node::*;
use markdown::{to_mdast, ParseOptions};

use crate::server::application::AppError;

use super::{NodeRanges, NodeType};

pub struct MdParser;

impl MdParser {
    /// Parse a markdown string into per-`NodeType` byte ranges.
    pub fn parse(markdown_string: &str) -> Result<NodeRanges, AppError> {
        let root = to_mdast(markdown_string, &ParseOptions::gfm())
            .map_err(|err| AppError::Validation(format!("darn markdown parse: {err}")))?;
        let mut ranges = NodeRanges::new();

        if let Node::Root(root_node) = root {
            for child in &root_node.children {
                Self::visit_node(child, &mut ranges);
            }
        }

        Self::add_word_ranges(markdown_string, &mut ranges);
        Self::add_sentence_ranges(markdown_string, &mut ranges);

        ranges.deduplicate_ranges();
        Ok(ranges)
    }

    fn visit_node(node: &Node, ranges: &mut NodeRanges) {
        if let Some((kind, start, end)) = Self::extract_position(node) {
            ranges.add(kind, start, end);
        }

        if let Some(children) = node.children() {
            for child in children {
                Self::visit_node(child, ranges);
            }
        }
    }

    fn extract_position(node: &Node) -> Option<(NodeType, u32, u32)> {
        let (kind, position) = match node {
            Root(n) => (NodeType::Root, &n.position),
            Blockquote(n) => (NodeType::Blockquote, &n.position),
            FootnoteDefinition(n) => (NodeType::FootnoteDefinition, &n.position),
            MdxJsxFlowElement(n) => (NodeType::MdxJsxFlowElement, &n.position),
            List(n) => (NodeType::List, &n.position),
            MdxjsEsm(n) => (NodeType::MdxjsEsm, &n.position),
            Toml(n) => (NodeType::Toml, &n.position),
            Yaml(n) => (NodeType::Yaml, &n.position),
            Break(n) => (NodeType::Break, &n.position),
            InlineCode(n) => (NodeType::InlineCode, &n.position),
            InlineMath(n) => (NodeType::InlineMath, &n.position),
            Delete(n) => (NodeType::Delete, &n.position),
            Emphasis(n) => (NodeType::Emphasis, &n.position),
            MdxTextExpression(n) => (NodeType::MdxTextExpression, &n.position),
            FootnoteReference(n) => (NodeType::FootnoteReference, &n.position),
            Html(n) => (NodeType::Html, &n.position),
            Image(n) => (NodeType::Image, &n.position),
            ImageReference(n) => (NodeType::ImageReference, &n.position),
            MdxJsxTextElement(n) => (NodeType::MdxJsxTextElement, &n.position),
            Link(n) => (NodeType::Link, &n.position),
            LinkReference(n) => (NodeType::LinkReference, &n.position),
            Strong(n) => (NodeType::Strong, &n.position),
            Text(n) => (NodeType::Text, &n.position),
            Code(n) => (NodeType::Code, &n.position),
            Math(n) => (NodeType::Math, &n.position),
            MdxFlowExpression(n) => (NodeType::MdxFlowExpression, &n.position),
            Heading(n) => (NodeType::Heading, &n.position),
            Table(n) => (NodeType::Table, &n.position),
            ThematicBreak(n) => (NodeType::ThematicBreak, &n.position),
            TableRow(n) => (NodeType::TableRow, &n.position),
            TableCell(n) => (NodeType::TableCell, &n.position),
            ListItem(n) => (NodeType::ListItem, &n.position),
            Definition(n) => (NodeType::Definition, &n.position),
            Paragraph(n) => (NodeType::Paragraph, &n.position),
        };

        position
            .as_ref()
            .map(|pos| (kind, pos.start.offset as u32, pos.end.offset as u32))
    }

    fn add_word_ranges(markdown_string: &str, ranges: &mut NodeRanges) {
        let bytes = markdown_string.as_bytes();
        let mut start = 0;

        while start < bytes.len() {
            while bytes.get(start).is_some_and(u8::is_ascii_whitespace) {
                start += 1;
            }
            if start >= bytes.len() {
                break;
            }
            let mut end = start;
            while bytes.get(end).is_some_and(|b| !b.is_ascii_whitespace()) {
                end += 1;
            }
            ranges.add(NodeType::Word, start as u32, end as u32);
            start = end;
        }
    }

    fn add_sentence_ranges(markdown_string: &str, ranges: &mut NodeRanges) {
        let sentence_endings = ['.', '!', '?'];
        let mut start = 0;
        for (idx, ch) in markdown_string.char_indices() {
            if sentence_endings.contains(&ch) {
                let end = idx + ch.len_utf8();
                ranges.add(NodeType::Sentence, start as u32, end as u32);
                start = end + 1;
            }
        }
        if start < markdown_string.len() {
            ranges.add(
                NodeType::Sentence,
                start as u32,
                markdown_string.len() as u32,
            );
        }
    }
}
