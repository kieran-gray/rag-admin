use std::collections::HashMap;

use super::search_space::{Observation, Parameter, SearchSpace, Trial, Value};

const GAMMA: f32 = 0.25;
const N_CANDIDATES: usize = 24;
pub const WARMUP_TRIALS: usize = 10;
const PERTURB_FRACTION: f64 = 0.15;

pub struct Tpe {
    space: SearchSpace,
    rng_state: u64,
    history: Vec<Observation>,
    next_trial_id: u32,
}

impl Tpe {
    pub fn new(space: SearchSpace, seed: u64) -> Self {
        let seed = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self {
            space,
            rng_state: seed,
            history: Vec::new(),
            next_trial_id: 0,
        }
    }

    pub fn observe(&mut self, observations: &[Observation]) {
        self.history.extend_from_slice(observations);
    }

    pub fn history(&self) -> &[Observation] {
        &self.history
    }

    pub fn skip_trial_ids(&mut self, count: u32) {
        if count > self.next_trial_id {
            self.next_trial_id = count;
        }
    }

    pub fn propose(&mut self, n: usize) -> Vec<Trial> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let params = self.propose_one();
            out.push(Trial {
                trial_id: self.next_trial_id,
                params,
            });
            self.next_trial_id += 1;
        }
        out
    }

    fn propose_one(&mut self) -> HashMap<String, Value> {
        if self.history.len() < WARMUP_TRIALS {
            return self.sample_from_prior();
        }

        let (good, bad): (Vec<Observation>, Vec<Observation>) = {
            let (g, b) = partition_by_fitness(&self.history, GAMMA);
            (
                g.into_iter().cloned().collect(),
                b.into_iter().cloned().collect(),
            )
        };
        if good.is_empty() || bad.is_empty() {
            return self.sample_from_prior();
        }

        let descriptors: Vec<Parameter> = self.space.parameters().to_vec();

        let mut best_score = f64::MIN;
        let mut best: HashMap<String, Value> = HashMap::new();
        for _ in 0..N_CANDIDATES {
            let candidate = self.sample_from_good(&descriptors, &good);
            let mut ratio = 0.0f64;
            for p in &descriptors {
                let leaf = match p {
                    Parameter::Conditional {
                        gate_parameter,
                        gate_value,
                        inner,
                    } => {
                        if candidate.get(gate_parameter) != Some(gate_value) {
                            continue;
                        }
                        inner.as_ref()
                    }
                    other => other,
                };
                let Some(value) = candidate.get(leaf.name()) else {
                    continue;
                };
                let l = density_of(&good, leaf, value);
                let g = density_of(&bad, leaf, value);
                if l > 0.0 && g > 0.0 {
                    ratio += (l / g).ln();
                }
            }
            if ratio > best_score {
                best_score = ratio;
                best = candidate;
            }
        }
        if best.is_empty() {
            self.sample_from_prior()
        } else {
            best
        }
    }

    fn sample_from_prior(&mut self) -> HashMap<String, Value> {
        let mut params: HashMap<String, Value> = HashMap::new();
        let descriptors: Vec<Parameter> = self.space.parameters().to_vec();
        for p in descriptors {
            match &p {
                Parameter::Conditional {
                    gate_parameter,
                    gate_value,
                    inner,
                } => {
                    if params.get(gate_parameter) == Some(gate_value) {
                        let name = inner.name().to_string();
                        let value = self.sample_param(inner);
                        params.insert(name, value);
                    }
                }
                _ => {
                    let name = p.name().to_string();
                    let value = self.sample_param(&p);
                    params.insert(name, value);
                }
            }
        }
        params
    }

    fn sample_from_good(
        &mut self,
        descriptors: &[Parameter],
        good: &[Observation],
    ) -> HashMap<String, Value> {
        if good.is_empty() {
            return self.sample_from_prior();
        }
        let pivot_idx = (self.next_u64() as usize) % good.len();
        let Some(pivot_obs) = good.get(pivot_idx) else {
            return self.sample_from_prior();
        };
        let pivot = pivot_obs.params.clone();

        let mut out: HashMap<String, Value> = HashMap::new();
        for p in descriptors {
            match p {
                Parameter::Conditional {
                    gate_parameter,
                    gate_value,
                    inner,
                } => {
                    if out.get(gate_parameter) != Some(gate_value) {
                        continue;
                    }
                    let name = inner.name().to_string();
                    let value = self.perturb_or_sample(inner, pivot.get(&name));
                    out.insert(name, value);
                }
                _ => {
                    let name = p.name().to_string();
                    let value = self.perturb_or_sample(p, pivot.get(&name));
                    out.insert(name, value);
                }
            }
        }
        out
    }

    fn perturb_or_sample(&mut self, p: &Parameter, pivot_value: Option<&Value>) -> Value {
        match p {
            Parameter::Categorical { values, .. } => {
                if values.is_empty() {
                    return Value::String(String::new());
                }
                let switch_prob = 1.0 / values.len() as f64;
                if let Some(Value::String(current)) = pivot_value {
                    if self.next_f64() >= switch_prob {
                        return Value::String(current.clone());
                    }
                }
                let idx = (self.next_u64() as usize) % values.len();
                Value::String(values.get(idx).cloned().unwrap_or_default())
            }
            Parameter::IntRange {
                low,
                high,
                log_scale,
                ..
            } => {
                let pivot_n = pivot_value
                    .and_then(|v| v.as_int())
                    .unwrap_or((*low + *high) / 2);
                Value::Int(self.perturb_int(pivot_n, *low, *high, *log_scale))
            }
            Parameter::Float {
                low,
                high,
                log_scale,
                ..
            } => {
                let pivot_f = pivot_value
                    .and_then(|v| v.as_float())
                    .unwrap_or((*low + *high) / 2.0);
                Value::Float(self.perturb_float(pivot_f, *low, *high, *log_scale))
            }
            Parameter::Conditional { inner, .. } => self.perturb_or_sample(inner, pivot_value),
        }
    }

    fn perturb_float(&mut self, pivot: f64, low: f64, high: f64, log_scale: bool) -> f64 {
        if high <= low {
            return low;
        }
        let g = self.next_gaussian();
        if log_scale && low > 0.0 && pivot > 0.0 {
            let log_lo = low.ln();
            let log_hi = high.ln();
            let step = PERTURB_FRACTION * (log_hi - log_lo);
            let new_log = (pivot.ln() + g * step).clamp(log_lo, log_hi);
            new_log.exp()
        } else {
            let step = PERTURB_FRACTION * (high - low);
            (pivot + g * step).clamp(low, high)
        }
    }

    fn perturb_int(&mut self, pivot: i64, low: i64, high: i64, log_scale: bool) -> i64 {
        if high <= low {
            return low;
        }
        let g = self.next_gaussian();
        if log_scale && low > 0 && pivot > 0 {
            let log_lo = (low as f64).ln();
            let log_hi = (high as f64).ln();
            let step = PERTURB_FRACTION * (log_hi - log_lo);
            let new_log = ((pivot as f64).ln() + g * step).clamp(log_lo, log_hi);
            new_log.exp().round().clamp(low as f64, high as f64) as i64
        } else {
            let step = PERTURB_FRACTION * (high - low) as f64;
            (pivot as f64 + g * step)
                .round()
                .clamp(low as f64, high as f64) as i64
        }
    }

    fn sample_param(&mut self, p: &Parameter) -> Value {
        match p {
            Parameter::Categorical { values, .. } => {
                let idx = (self.next_u64() as usize) % values.len().max(1);
                Value::String(values.get(idx).cloned().unwrap_or_default())
            }
            Parameter::IntRange {
                low,
                high,
                log_scale,
                ..
            } => {
                let lo = *low;
                let hi = *high;
                if hi <= lo {
                    return Value::Int(lo);
                }
                if *log_scale && lo > 0 {
                    let log_lo = (lo as f64).ln();
                    let log_hi = (hi as f64).ln();
                    let u = self.next_f64();
                    let v = (log_lo + u * (log_hi - log_lo)).exp().round() as i64;
                    Value::Int(v.clamp(lo, hi))
                } else {
                    let u = self.next_f64();
                    let v = lo + (u * (hi - lo + 1) as f64).floor() as i64;
                    Value::Int(v.clamp(lo, hi))
                }
            }
            Parameter::Float {
                low,
                high,
                log_scale,
                ..
            } => {
                let lo = *low;
                let hi = *high;
                if hi <= lo {
                    return Value::Float(lo);
                }
                if *log_scale && lo > 0.0 {
                    let log_lo = lo.ln();
                    let log_hi = hi.ln();
                    let u = self.next_f64();
                    Value::Float((log_lo + u * (log_hi - log_lo)).exp())
                } else {
                    let u = self.next_f64();
                    Value::Float(lo + u * (hi - lo))
                }
            }
            Parameter::Conditional { inner, .. } => self.sample_param(inner),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-300);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

pub fn partition_by_fitness(
    history: &[Observation],
    gamma: f32,
) -> (Vec<&Observation>, Vec<&Observation>) {
    if history.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut sorted: Vec<&Observation> = history.iter().collect();
    sorted.sort_by(|a, b| {
        b.fitness
            .composite
            .partial_cmp(&a.fitness.composite)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n_good = ((sorted.len() as f32) * gamma).ceil() as usize;
    let n_good = n_good.clamp(1, sorted.len().saturating_sub(1).max(1));
    let (good, bad) = sorted.split_at(n_good);
    (good.to_vec(), bad.to_vec())
}

fn density_of(subset: &[Observation], param: &Parameter, value: &Value) -> f64 {
    match param {
        Parameter::Categorical { name, values } => {
            density_categorical(subset, name, value, values.len())
        }
        Parameter::IntRange {
            name, log_scale, ..
        } => density_numeric(subset, name, value, *log_scale),
        Parameter::Float {
            name, log_scale, ..
        } => density_numeric(subset, name, value, *log_scale),
        Parameter::Conditional { inner, .. } => density_of(subset, inner, value),
    }
}

fn density_categorical(
    subset: &[Observation],
    name: &str,
    value: &Value,
    cardinality: usize,
) -> f64 {
    let target = match value {
        Value::String(s) => s.as_str(),
        _ => return 1e-9,
    };
    let mut count = 0usize;
    let mut total = 0usize;
    for obs in subset {
        if let Some(Value::String(s)) = obs.params.get(name) {
            total += 1;
            if s == target {
                count += 1;
            }
        }
    }

    (count as f64 + 1.0) / (total as f64 + cardinality.max(1) as f64)
}

fn density_numeric(subset: &[Observation], name: &str, value: &Value, log_scale: bool) -> f64 {
    let query = match value.as_float() {
        Some(f) => f,
        None => return 1e-9,
    };
    let xs: Vec<f64> = subset
        .iter()
        .filter_map(|o| {
            let raw = o.params.get(name).and_then(|v| v.as_float())?;
            if log_scale {
                if raw > 0.0 {
                    Some(raw.ln())
                } else {
                    None
                }
            } else {
                Some(raw)
            }
        })
        .collect();
    if xs.is_empty() {
        return 1e-9;
    }
    let q = if log_scale && query > 0.0 {
        query.ln()
    } else if log_scale {
        return 1e-9;
    } else {
        query
    };

    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let stddev = var.sqrt().max(1e-6);

    let h = (1.06_f64 * stddev * n.powf(-0.2)).max(1e-6);

    let coeff = 1.0 / (n * h * (2.0 * std::f64::consts::PI).sqrt());
    let sum: f64 = xs
        .iter()
        .map(|x| {
            let z = (q - x) / h;
            (-0.5 * z * z).exp()
        })
        .sum();
    (coeff * sum).max(1e-9)
}

#[cfg(test)]
mod tests {
    use super::super::search_space::{Fitness, Observation, Parameter, SearchSpace, Value};
    use super::*;

    fn space() -> SearchSpace {
        SearchSpace::new(vec![
            Parameter::IntRange {
                name: "top_k".into(),
                low: 1,
                high: 20,
                log_scale: false,
            },
            Parameter::Float {
                name: "min_score".into(),
                low: 0.0,
                high: 1.0,
                log_scale: false,
            },
        ])
    }

    fn make_obs(trial_id: u32, params: HashMap<String, Value>, composite: f32) -> Observation {
        Observation {
            trial_id,
            params,
            fitness: Fitness {
                composite,
                composite_ci: (composite - 0.02, composite + 0.02),
                recall: composite,
                precision: composite,
                iou: composite,
                precision_omega: composite,
                cost: 100.0,
                judge_score: None,
            },
        }
    }

    #[test]
    fn proposes_trials_in_bounds() {
        let mut tpe = Tpe::new(space(), 1234);
        let trials = tpe.propose(5);
        assert_eq!(trials.len(), 5);
        for t in &trials {
            let k = t.params["top_k"].as_int().unwrap();
            assert!((1..=20).contains(&k));
            let s = t.params["min_score"].as_float().unwrap();
            assert!((0.0..=1.0).contains(&s));
        }
    }

    #[test]
    fn trial_ids_are_sequential() {
        let mut tpe = Tpe::new(space(), 1234);
        let trials = tpe.propose(3);
        assert_eq!(trials[0].trial_id, 0);
        assert_eq!(trials[1].trial_id, 1);
        assert_eq!(trials[2].trial_id, 2);
    }

    #[test]
    fn observations_are_remembered() {
        let mut tpe = Tpe::new(space(), 1234);
        let trials = tpe.propose(3);
        let obs: Vec<Observation> = trials
            .iter()
            .map(|t| make_obs(t.trial_id, t.params.clone(), 0.5))
            .collect();
        tpe.observe(&obs);
        assert_eq!(tpe.history().len(), 3);
    }

    #[test]
    fn partition_by_fitness_promotes_top_quantile() {
        let observations: Vec<Observation> = (0..20)
            .map(|i| make_obs(i, HashMap::new(), i as f32 / 20.0))
            .collect();
        let (good, bad) = partition_by_fitness(&observations, GAMMA);

        assert_eq!(good.len(), 5);
        assert_eq!(bad.len(), 15);
        let min_good = good
            .iter()
            .map(|o| o.fitness.composite)
            .fold(f32::INFINITY, f32::min);
        let max_bad = bad
            .iter()
            .map(|o| o.fitness.composite)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(min_good > max_bad);
    }

    #[test]
    fn density_categorical_favours_observed_values() {
        let param = Parameter::Categorical {
            name: "strategy".into(),
            values: vec!["section".into(), "bert".into()],
        };
        let good: Vec<Observation> = (0..10)
            .map(|i| {
                let mut params = HashMap::new();
                params.insert("strategy".into(), Value::String("section".into()));
                make_obs(i, params, 0.9)
            })
            .collect();
        let bad: Vec<Observation> = (0..10)
            .map(|i| {
                let mut params = HashMap::new();
                params.insert("strategy".into(), Value::String("bert".into()));
                make_obs(10 + i, params, 0.2)
            })
            .collect();
        let l_section = density_of(&good, &param, &Value::String("section".into()));
        let g_section = density_of(&bad, &param, &Value::String("section".into()));
        assert!(
            l_section > g_section,
            "section should be denser in good (got l={l_section}, g={g_section})"
        );
    }

    #[test]
    fn density_numeric_peaks_near_observed_cluster() {
        let param = Parameter::Float {
            name: "x".into(),
            low: 0.0,
            high: 1.0,
            log_scale: false,
        };
        let good: Vec<Observation> = (0..20)
            .map(|i| {
                let mut params = HashMap::new();
                let x = 0.7 + 0.02 * ((i as f64) - 10.0) / 10.0;
                params.insert("x".into(), Value::Float(x));
                make_obs(i, params, 0.9)
            })
            .collect();
        let near_peak = density_of(&good, &param, &Value::Float(0.7));
        let far = density_of(&good, &param, &Value::Float(0.1));
        assert!(
            near_peak > 5.0 * far,
            "density should peak near 0.7: near={near_peak}, far={far}"
        );
    }

    #[test]
    fn tpe_converges_on_1d_quadratic() {
        let landscape = |x: f64| -> f32 { (1.0 - (x - 0.7).powi(2)) as f32 };
        let space = SearchSpace::new(vec![Parameter::Float {
            name: "x".into(),
            low: 0.0,
            high: 1.0,
            log_scale: false,
        }]);
        let mut tpe = Tpe::new(space, 7);

        let mut best = f32::MIN;

        for _ in 0..5 {
            let trials = tpe.propose(10);
            let obs: Vec<Observation> = trials
                .iter()
                .map(|t| {
                    let x = t.params["x"].as_float().unwrap();
                    let composite = landscape(x);
                    make_obs(t.trial_id, t.params.clone(), composite)
                })
                .collect();
            for o in &obs {
                if o.fitness.composite > best {
                    best = o.fitness.composite;
                }
            }
            tpe.observe(&obs);
        }

        assert!(
            best > 0.99,
            "TPE best after 50 trials should be near optimum (1.0), got {best}"
        );
    }

    #[test]
    fn tpe_converges_on_categorical_bias() {
        let space = SearchSpace::new(vec![Parameter::Categorical {
            name: "strategy".into(),
            values: vec!["good".into(), "bad".into()],
        }]);
        let mut tpe = Tpe::new(space, 11);

        let mut last_batch_good = 0usize;
        for batch in 0..6 {
            let trials = tpe.propose(10);
            let obs: Vec<Observation> = trials
                .iter()
                .map(|t| {
                    let s = t.params["strategy"].as_string().unwrap();
                    let composite = if s == "good" { 0.9 } else { 0.2 };
                    make_obs(t.trial_id, t.params.clone(), composite)
                })
                .collect();
            if batch == 5 {
                last_batch_good = obs
                    .iter()
                    .filter(|o| {
                        matches!(o.params.get("strategy"), Some(Value::String(s)) if s == "good")
                    })
                    .count();
            }
            tpe.observe(&obs);
        }
        assert!(
            last_batch_good >= 7,
            "after warmup TPE should pick the better category most of the time; got {last_batch_good}/10"
        );
    }
}
