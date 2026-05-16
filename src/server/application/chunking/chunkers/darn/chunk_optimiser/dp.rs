use std::cmp::Ordering;

use crate::server::application::AppError;

/// Picks the best successor `j` for index `i` over the range `i+1..=i+n`.
/// Ties break toward the *largest* jump, which keeps chunks as long as possible.
fn best_successor(i: usize, punishments: &[usize], dp_cost: &[usize], n: usize) -> (usize, usize) {
    (i + 1..=i + n)
        .map(|j| (punishments[i] + dp_cost[j], j))
        .min_by(|(cost_a, j_a), (cost_b, j_b)| match cost_a.cmp(cost_b) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            Ordering::Equal => j_b.cmp(j_a),
        })
        .unwrap()
}

fn build_dp_tables(punishments: &[usize], n: usize) -> (Vec<usize>, Vec<Option<usize>>) {
    let len = punishments.len();
    let inf = usize::MAX;
    let mut dp_cost = vec![inf; len];
    let mut next = vec![None; len];

    for i in (0..len).rev() {
        if i + n >= len {
            dp_cost[i] = punishments[i];
            next[i] = None;
        } else {
            let (cost, j) = best_successor(i, punishments, &dp_cost, n);
            dp_cost[i] = cost;
            next[i] = Some(j);
        }
    }

    (dp_cost, next)
}

fn reconstruct_path(next: &[Option<usize>]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut i = 0;
    loop {
        path.push(i);
        match next[i] {
            Some(j) => i = j,
            None => break,
        }
    }
    path
}

/// Find optimal chunk start indices given a punishment vector and the maximum
/// chunk size `n` (in punishment-vector units, i.e. bytes or tokens depending
/// on caller). Empty input or `n == 0` yields an error.
pub fn cheapest_path_indices(punishments: &[usize], n: usize) -> Result<Vec<usize>, AppError> {
    if punishments.is_empty() {
        return Err(AppError::Validation("darn: empty punishment vector".into()));
    }
    if n == 0 {
        return Err(AppError::Validation(
            "darn: max_chunk_size must be > 0".into(),
        ));
    }

    let (_dp_cost, next) = build_dp_tables(punishments, n);
    Ok(reconstruct_path(&next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cell_returns_single_index() {
        let path = cheapest_path_indices(&[0], 4).unwrap();
        assert_eq!(path, vec![0]);
    }

    #[test]
    fn prefers_larger_jumps_on_ties() {
        // All-zero punishments: every cut costs the same, so the optimiser
        // should jump as far as possible each step.
        let path = cheapest_path_indices(&[0; 10], 4).unwrap();
        assert_eq!(path, vec![0, 4, 8]);
    }

    #[test]
    fn avoids_high_penalty_cells() {
        let mut p = vec![1usize; 12];
        // Cell 3 is very expensive — DP should land on neighbours instead.
        p[3] = 1_000;
        let path = cheapest_path_indices(&p, 4).unwrap();
        assert!(
            !path.contains(&3),
            "path should avoid index 3, got {path:?}"
        );
    }

    #[test]
    fn rejects_zero_chunk_size() {
        assert!(cheapest_path_indices(&[0, 0, 0], 0).is_err());
    }
}
