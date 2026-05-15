use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParetoPoint {
    pub quality: f32,
    pub cost: f32,
}

pub fn pareto_frontier(points: &[ParetoPoint]) -> Vec<usize> {
    let mut out = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        let dominated = points.iter().enumerate().any(|(j, q)| {
            i != j
                && q.quality >= p.quality
                && q.cost <= p.cost
                && (q.quality > p.quality || q.cost < p.cost)
        });
        if !dominated {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_all_points_when_no_dominance() {
        let points = vec![
            ParetoPoint { quality: 0.7, cost: 100.0 },
            ParetoPoint { quality: 0.8, cost: 200.0 },
            ParetoPoint { quality: 0.9, cost: 300.0 },
        ];
        let frontier = pareto_frontier(&points);
        assert_eq!(frontier, vec![0, 1, 2]);
    }

    #[test]
    fn drops_strictly_dominated() {
        let points = vec![
            ParetoPoint { quality: 0.9, cost: 100.0 },
            ParetoPoint { quality: 0.7, cost: 200.0 },
            ParetoPoint { quality: 0.8, cost: 50.0 },
        ];
        let frontier = pareto_frontier(&points);
        assert!(frontier.contains(&0));
        assert!(frontier.contains(&2));
        assert!(!frontier.contains(&1));
    }

    #[test]
    fn keeps_ties() {
        let points = vec![
            ParetoPoint { quality: 0.8, cost: 100.0 },
            ParetoPoint { quality: 0.8, cost: 100.0 },
        ];
        let frontier = pareto_frontier(&points);
        assert_eq!(frontier.len(), 2);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(pareto_frontier(&[]).is_empty());
    }
}
