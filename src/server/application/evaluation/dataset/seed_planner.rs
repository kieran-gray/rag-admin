use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use uuid::Uuid;

use crate::server::application::AppError;
use crate::server::domain::comprehension::insight::Insight;
use crate::server::domain::comprehension::map::read_model::DocumentMapDetail;
use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::server::domain::comprehension::observation::Observation;
use crate::server::domain::comprehension::role::SuggestedRole;
use crate::server::domain::comprehension::span::Span;
use crate::server::domain::comprehension::thread::Thread;
use crate::server::domain::evaluation::dataset::events::AxisWeight;
use crate::server::domain::evaluation::question::{
    is_combination_valid, CognitiveOperation, EvidenceKind, QuestionDimensions,
};

pub struct UsedSeedRegistry<'a> {
    pub seen_observation_ids: Vec<Uuid>,
    pub seen_thread_ids: Vec<Uuid>,
    pub seen_insight_ids: Vec<Uuid>,
    pub _phantom: PhantomData<&'a ()>,
}

impl UsedSeedRegistry<'_> {
    pub fn empty() -> Self {
        Self {
            seen_observation_ids: Vec::new(),
            seen_thread_ids: Vec::new(),
            seen_insight_ids: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeedItem {
    pub map_item_ref: MapItemRef,
    pub kind: String,
    pub summary: String,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub struct QuestionSeed {
    pub dimensions: QuestionDimensions,
    pub items: Vec<SeedItem>,
    pub spans: Vec<Span>,
    pub anti_evidence: Vec<SeedItem>,
}

pub struct QuestionPlanTarget {
    pub operation: CognitiveOperation,
    pub evidence: EvidenceKind,
    pub role: SuggestedRole,
}

pub fn pick_axes(
    weight_matrix: &[AxisWeight],
    accepted_by_axes: &HashMap<String, u32>,
    target_total: u32,
    rng_seed: u64,
) -> Option<(CognitiveOperation, EvidenceKind)> {
    let total_weight: u64 = weight_matrix.iter().map(|w| w.weight as u64).sum();
    if total_weight == 0 {
        return None;
    }
    let mut best: Option<((CognitiveOperation, EvidenceKind), i64)> = None;
    for w in weight_matrix {
        let op = CognitiveOperation::parse(&w.operation).ok()?;
        let ev = EvidenceKind::parse(&w.evidence).ok()?;
        if !is_combination_valid(op, ev) {
            continue;
        }
        let key = format!("{}:{}", op.as_str(), ev.as_str());
        let target = (target_total as u64 * w.weight as u64 / total_weight) as i64;
        let accepted = *accepted_by_axes.get(&key).unwrap_or(&0) as i64;
        let deficit = target - accepted;
        let tiebreak = (rng_seed.wrapping_mul(31) ^ hash_key(&key)) as i64 % 1024;
        let score = deficit * 1024 + (tiebreak / 8);
        match best {
            None => best = Some(((op, ev), score)),
            Some((_, b)) if score > b => best = Some(((op, ev), score)),
            _ => {}
        }
    }
    best.map(|(k, _)| k)
}

fn hash_key(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn pick_from<T: Copy>(pool: &[T], rng_seed: u64) -> Option<T> {
    if pool.is_empty() {
        return None;
    }
    pool.get((rng_seed as usize) % pool.len()).copied()
}

pub fn pick_role(roles: &[SuggestedRole], rng_seed: u64) -> SuggestedRole {
    if roles.is_empty() {
        return SuggestedRole {
            name: "general reader".into(),
            focus: "the document as written".into(),
        };
    }
    let idx = (rng_seed as usize) % roles.len();
    roles.get(idx).cloned().unwrap_or_else(|| SuggestedRole {
        name: "general reader".into(),
        focus: "the document as written".into(),
    })
}

pub fn plan_seed(
    target: &QuestionPlanTarget,
    detail: &DocumentMapDetail,
    used: &UsedSeedRegistry<'_>,
    rng_seed: u64,
) -> Result<QuestionSeed, AppError> {
    let dimensions = QuestionDimensions {
        operation: target.operation,
        evidence: target.evidence,
        role: target.role.name.clone(),
    };

    match (target.operation, target.evidence) {
        (CognitiveOperation::Adversarial, _) => plan_adversarial_seed(dimensions, detail, rng_seed),
        (_, EvidenceKind::Observation) => {
            plan_observation_seed(dimensions, &detail.observations, used, rng_seed)
        }
        (_, EvidenceKind::Thread) => plan_thread_seed(
            dimensions,
            &detail.threads,
            &detail.observations,
            used,
            rng_seed,
        ),
        (_, EvidenceKind::Insight) => plan_insight_seed(
            dimensions,
            &detail.insights,
            &detail.threads,
            &detail.observations,
            used,
            rng_seed,
        ),
        (_, EvidenceKind::Connection) => Err(AppError::Validation(
            "Connection-based seeds are not yet implemented in single-document scope".into(),
        )),
    }
}

fn plan_observation_seed(
    dimensions: QuestionDimensions,
    observations: &[Observation],
    used: &UsedSeedRegistry<'_>,
    rng_seed: u64,
) -> Result<QuestionSeed, AppError> {
    if observations.is_empty() {
        return Err(AppError::Validation("no observations available".into()));
    }
    let unused: Vec<&Observation> = observations
        .iter()
        .filter(|o| !used.seen_observation_ids.contains(&o.observation_id))
        .collect();
    let pool: Vec<&Observation> = if unused.is_empty() {
        observations.iter().collect()
    } else {
        unused
    };
    let chosen = pick_from(&pool, rng_seed)
        .ok_or_else(|| AppError::Validation("observation pool empty after filtering".into()))?;
    let item = observation_to_seed(chosen);
    let spans = item.spans.clone();
    Ok(QuestionSeed {
        dimensions,
        items: vec![item],
        spans,
        anti_evidence: Vec::new(),
    })
}

fn plan_thread_seed(
    dimensions: QuestionDimensions,
    threads: &[Thread],
    observations: &[Observation],
    used: &UsedSeedRegistry<'_>,
    rng_seed: u64,
) -> Result<QuestionSeed, AppError> {
    if threads.is_empty() {
        return Err(AppError::Validation("no threads available".into()));
    }
    let unused: Vec<&Thread> = threads
        .iter()
        .filter(|t| !used.seen_thread_ids.contains(&t.thread_id))
        .collect();
    let pool: Vec<&Thread> = if unused.is_empty() {
        threads.iter().collect()
    } else {
        unused
    };
    let chosen = pick_from(&pool, rng_seed)
        .ok_or_else(|| AppError::Validation("thread pool empty after filtering".into()))?;
    let item = thread_to_seed(chosen);
    let mut spans = item.spans.clone();
    let mut items = vec![item];
    for evidence_ref in &chosen.evidence {
        if let MapItemRef::Observation { observation_id, .. } = evidence_ref {
            if let Some(o) = observations
                .iter()
                .find(|o| o.observation_id == *observation_id)
            {
                let obs_item = observation_to_seed(o);
                spans.extend(obs_item.spans.iter().copied());
                items.push(obs_item);
            }
        }
    }
    Ok(QuestionSeed {
        dimensions,
        items,
        spans,
        anti_evidence: Vec::new(),
    })
}

fn plan_insight_seed(
    dimensions: QuestionDimensions,
    insights: &[Insight],
    threads: &[Thread],
    observations: &[Observation],
    used: &UsedSeedRegistry<'_>,
    rng_seed: u64,
) -> Result<QuestionSeed, AppError> {
    if insights.is_empty() {
        return Err(AppError::Validation("no insights available".into()));
    }
    let unused: Vec<&Insight> = insights
        .iter()
        .filter(|i| !used.seen_insight_ids.contains(&i.insight_id))
        .collect();
    let pool: Vec<&Insight> = if unused.is_empty() {
        insights.iter().collect()
    } else {
        unused
    };
    let chosen = pick_from(&pool, rng_seed)
        .ok_or_else(|| AppError::Validation("insight pool empty after filtering".into()))?;
    let item = insight_to_seed(chosen);
    let mut spans = item.spans.clone();
    let mut items = vec![item];
    for evidence_ref in &chosen.evidence {
        match evidence_ref {
            MapItemRef::Observation { observation_id, .. } => {
                if let Some(o) = observations
                    .iter()
                    .find(|o| o.observation_id == *observation_id)
                {
                    let obs_item = observation_to_seed(o);
                    spans.extend(obs_item.spans.iter().copied());
                    items.push(obs_item);
                }
            }
            MapItemRef::Thread { thread_id, .. } => {
                if let Some(t) = threads.iter().find(|t| t.thread_id == *thread_id) {
                    let thread_item = thread_to_seed(t);
                    spans.extend(thread_item.spans.iter().copied());
                    items.push(thread_item);
                }
            }
            MapItemRef::Insight { .. } | MapItemRef::Connection { .. } => {}
        }
    }
    Ok(QuestionSeed {
        dimensions,
        items,
        spans,
        anti_evidence: Vec::new(),
    })
}

fn plan_adversarial_seed(
    dimensions: QuestionDimensions,
    detail: &DocumentMapDetail,
    rng_seed: u64,
) -> Result<QuestionSeed, AppError> {
    let mut anti = Vec::new();
    if let Some(thread) = pick_from(&detail.threads.iter().collect::<Vec<_>>(), rng_seed) {
        anti.push(thread_to_seed(thread));
    }
    if let Some(obs) = pick_from(
        &detail.observations.iter().collect::<Vec<_>>(),
        rng_seed.wrapping_mul(7),
    ) {
        anti.push(observation_to_seed(obs));
    }
    if anti.is_empty() {
        return Err(AppError::Validation(
            "no map material available for adversarial seed".into(),
        ));
    }
    Ok(QuestionSeed {
        dimensions,
        items: Vec::new(),
        spans: Vec::new(),
        anti_evidence: anti,
    })
}

fn observation_to_seed(o: &Observation) -> SeedItem {
    SeedItem {
        map_item_ref: MapItemRef::Observation {
            map_id: Uuid::nil(),
            observation_id: o.observation_id,
        },
        kind: o.kind.clone(),
        summary: o.summary.clone(),
        spans: o.spans.clone(),
    }
}

fn thread_to_seed(t: &Thread) -> SeedItem {
    SeedItem {
        map_item_ref: MapItemRef::Thread {
            map_id: Uuid::nil(),
            thread_id: t.thread_id,
        },
        kind: t.kind.clone(),
        summary: t.summary.clone(),
        spans: t.spans.clone(),
    }
}

fn insight_to_seed(i: &Insight) -> SeedItem {
    SeedItem {
        map_item_ref: MapItemRef::Insight {
            map_id: Uuid::nil(),
            insight_id: i.insight_id,
        },
        kind: i.kind.clone(),
        summary: i.summary.clone(),
        spans: i.spans.clone(),
    }
}

pub fn rebind_seed_to_map(seed: &QuestionSeed, map_id: Uuid) -> Vec<MapItemRef> {
    seed.items
        .iter()
        .map(|item| match item.map_item_ref {
            MapItemRef::Observation { observation_id, .. } => MapItemRef::Observation {
                map_id,
                observation_id,
            },
            MapItemRef::Thread { thread_id, .. } => MapItemRef::Thread { map_id, thread_id },
            MapItemRef::Insight { insight_id, .. } => MapItemRef::Insight { map_id, insight_id },
            MapItemRef::Connection {
                corpus_map_id,
                connection_id,
            } => MapItemRef::Connection {
                corpus_map_id,
                connection_id,
            },
        })
        .collect()
}
