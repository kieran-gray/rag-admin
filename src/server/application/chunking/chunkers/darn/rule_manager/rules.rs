use super::punishments::{
    const_punishment, inverse_triangular_punishment, reverse_linear_punishment, PunishmentFn,
};
use crate::server::application::chunking::chunkers::darn::md_parser::NodeType;

/// A rule defines per-byte penalty functions for a given `NodeType`. Inside a
/// matching range, `on_punishment(on_scale)` is added; outside,
/// `off_punishment(off_scale)`.
#[derive(Clone)]
pub struct Rule {
    pub on_punishment: PunishmentFn,
    pub on_scale: usize,
    pub off_punishment: PunishmentFn,
    pub off_scale: usize,
    pub node_type: NodeType,
}

/// Default penalty table ported from cashewe/darn. The weights reflect upstream
/// tuning for typical English markdown.
pub static RULES: &[Rule] = &[
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Paragraph,
    },
    Rule {
        on_punishment: inverse_triangular_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Paragraph,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 100,
        off_punishment: reverse_linear_punishment,
        off_scale: 0,
        node_type: NodeType::Heading,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Blockquote,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Code,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 150,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Word,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 100,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Sentence,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::Table,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::TableRow,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 100,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::TableCell,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::List,
    },
    Rule {
        on_punishment: const_punishment,
        on_scale: 100,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::ListItem,
    },
    Rule {
        on_punishment: inverse_triangular_punishment,
        on_scale: 50,
        off_punishment: const_punishment,
        off_scale: 0,
        node_type: NodeType::List,
    },
];
