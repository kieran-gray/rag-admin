use std::fmt;

use enum_map::EnumMap;

use super::{NodeStartEnd, NodeType};

pub struct NodeRanges {
    pub ranges: EnumMap<NodeType, Vec<NodeStartEnd>>,
}

impl Default for NodeRanges {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRanges {
    pub fn new() -> Self {
        Self {
            ranges: EnumMap::default(),
        }
    }

    pub fn add(&mut self, kind: NodeType, start: u32, end: u32) {
        self.ranges[kind].push(NodeStartEnd { start, end });
    }

    pub fn deduplicate_ranges(&mut self) {
        for node_ranges in self.ranges.values_mut() {
            Self::merge_overlapping(node_ranges);
        }
    }

    fn merge_overlapping(node_ranges: &mut Vec<NodeStartEnd>) {
        if node_ranges.is_empty() {
            return;
        }

        node_ranges.sort_by_key(|r| r.start);

        let mut iter = node_ranges.iter();
        let Some(&first) = iter.next() else {
            return;
        };
        let mut merged = Vec::with_capacity(node_ranges.len());
        let mut current = first;
        for next in iter {
            if next.start <= current.end {
                current.end = current.end.max(next.end);
            } else {
                merged.push(current);
                current = *next;
            }
        }
        merged.push(current);
        *node_ranges = merged;
    }
}

impl fmt::Display for NodeRanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (node_type, ranges) in &self.ranges {
            writeln!(f, "{node_type:?}")?;
            for range in ranges {
                writeln!(f, "- {range}")?;
            }
        }
        Ok(())
    }
}
