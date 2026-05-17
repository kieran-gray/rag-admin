use super::rules::{Rule, RULES};
use crate::server::application::chunking::chunkers::darn::md_parser::{
    NodeRanges, NodeStartEnd, NodeType,
};

pub struct RuleManager;

impl RuleManager {
    /// Build the per-byte punishment vector by accumulating rules over the
    /// `NodeRanges` map. Each cell is the sum of every rule's contribution.
    pub fn build_punishment_vector(
        node_ranges: &NodeRanges,
        total_len: usize,
        rules: Option<&[Rule]>,
    ) -> Vec<usize> {
        let rules = rules.unwrap_or(RULES);
        let mut result = vec![0usize; total_len];

        for (node_type, ranges) in node_ranges.ranges.iter() {
            let matching = Self::rules_for(node_type, rules);
            if matching.is_empty() {
                continue;
            }
            for rule in matching {
                Self::apply_rule(rule, ranges, total_len, &mut result);
            }
        }

        result
    }

    fn apply_rule(rule: &Rule, ranges: &[NodeStartEnd], total_len: usize, output: &mut [usize]) {
        let mut cursor = 0usize;

        for r in ranges {
            let start = (r.start as usize).min(total_len);
            let end = (r.end as usize).min(total_len);

            if cursor < start {
                Self::apply_segment(rule.off_punishment, rule.off_scale, cursor, start, output);
            }

            if start < end {
                Self::apply_segment(rule.on_punishment, rule.on_scale, start, end, output);
            }

            cursor = end;
        }

        if cursor < total_len {
            Self::apply_segment(
                rule.off_punishment,
                rule.off_scale,
                cursor,
                total_len,
                output,
            );
        }
    }

    #[inline]
    fn apply_segment(
        f: fn(usize, usize, &mut [usize]),
        scale: usize,
        start: usize,
        end: usize,
        output: &mut [usize],
    ) {
        let len = end - start;
        let mut tmp = vec![0usize; len];
        f(scale, len, &mut tmp);
        let Some(target) = output.get_mut(start..end) else {
            return;
        };
        for (dst, val) in target.iter_mut().zip(tmp) {
            *dst += val;
        }
    }

    fn rules_for(node_type: NodeType, rules: &[Rule]) -> Vec<&Rule> {
        rules.iter().filter(|r| r.node_type == node_type).collect()
    }
}
