use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::domain::shared::Timestamp;
use crate::server::event_sourcing::Aggregate;
use crate::shared::{
    ChunkingVariant, EvaluationAutotuneRequest, EvaluationResultSplit, EvaluationRunOptions,
    OptimizationConfig,
};

use super::{
    commands::EvaluationRunCommand,
    events::{
        ChampionSelected, EvaluationRunEvent, RunCompleted, RunFailed, RunRequested, RungAdvanced,
        TrialProposed, VariantPrepared, VariantScored,
    },
    exceptions::EvaluationRunError,
    scoring_policy::ScoringPolicy,
};

use crate::server::domain::evaluation::optimizer::SearchBudget;
use crate::shared::OptimizationBudget;

const EVAL_RUN_NAMESPACE: Uuid = uuid::uuid!("b2e4f6a8-c0d2-4e6f-8012-3456789abcde");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvaluationRunStatus {
    Pending,
    Running { variants_completed: u32 },
    Completed,
    Failed { reason: String },
}

impl EvaluationRunStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed { .. })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running { .. } => "running",
            Self::Completed => "completed",
            Self::Failed { .. } => "failed",
        }
    }

    pub fn from_parts(
        status: &str,
        variants_completed: u32,
        failure_reason: Option<String>,
    ) -> Result<Self, String> {
        match status {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running { variants_completed }),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed {
                reason: failure_reason.unwrap_or_default(),
            }),
            other => Err(format!("unknown run status '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScoredVariantKey {
    pub variant_label: String,
    pub options: EvaluationRunOptions,
    pub split: EvaluationResultSplit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRun {
    pub run_id: Uuid,
    pub dataset_id: Uuid,
    pub pipeline_configuration_id: Uuid,
    pub document_id: Uuid,
    pub document_version: u32,
    pub variants: Vec<ChunkingVariant>,
    pub options: Vec<EvaluationRunOptions>,
    pub autotune_request: Option<EvaluationAutotuneRequest>,
    pub optimization: Option<crate::shared::OptimizationConfig>,
    pub scoring_policy: ScoringPolicy,
    pub prepared_labels: BTreeSet<String>,
    pub scored_keys: BTreeSet<ScoredVariantKey>,
    pub proposed_trials: BTreeSet<u32>,
    pub champion_trial_id: Option<u32>,
    pub status: EvaluationRunStatus,
    pub created_at: Timestamp,
}

impl EvaluationRun {
    pub fn compute_id(
        dataset_id: Uuid,
        pipeline_configuration_id: Uuid,
        variants: &[ChunkingVariant],
        options: &[EvaluationRunOptions],
        autotune_request: Option<&EvaluationAutotuneRequest>,
        optimization: Option<&OptimizationConfig>,
    ) -> Uuid {
        let key = serde_json::to_string(&(
            dataset_id,
            pipeline_configuration_id,
            variants,
            options,
            autotune_request,
            optimization,
        ))
        .unwrap_or_default();
        Uuid::new_v5(&EVAL_RUN_NAMESPACE, key.as_bytes())
    }

    fn from_requested(e: &RunRequested) -> Self {
        Self {
            run_id: e.run_id,
            dataset_id: e.dataset_id,
            pipeline_configuration_id: e.pipeline_configuration_id,
            document_id: e.document_id,
            document_version: e.document_version,
            variants: e.variants.clone(),
            options: e.options.clone(),
            autotune_request: e.autotune_request.clone(),
            optimization: e.optimization.clone(),
            scoring_policy: e.scoring_policy,
            prepared_labels: BTreeSet::new(),
            scored_keys: BTreeSet::new(),
            proposed_trials: BTreeSet::new(),
            champion_trial_id: None,
            status: EvaluationRunStatus::Pending,
            created_at: e.occurred_at.clone(),
        }
    }

    pub fn expected_score_count(&self) -> u32 {
        if let Some(opt) = &self.optimization {
            let budget: SearchBudget = match opt.budget {
                OptimizationBudget::Quick => SearchBudget::Quick,
                OptimizationBudget::Thorough => SearchBudget::Thorough,
                OptimizationBudget::Exhaustive => SearchBudget::Exhaustive,
            };
            let total_rung_scores: usize = budget.schedule().iter().map(|r| r.trials).sum();
            return (total_rung_scores + budget.holdout_top_n() + 1) as u32;
        }
        let combos = self.variants.len() * self.options.len();
        match &self.autotune_request {
            Some(req) => {
                let holdout = (req.holdout_top_n as usize).min(combos);
                (combos + holdout) as u32
            }
            None => combos as u32,
        }
    }

    pub fn expected_optimization_counts(&self) -> Option<(u32, u32, u32)> {
        let opt = self.optimization.as_ref()?;
        let budget: SearchBudget = match opt.budget {
            OptimizationBudget::Quick => SearchBudget::Quick,
            OptimizationBudget::Thorough => SearchBudget::Thorough,
            OptimizationBudget::Exhaustive => SearchBudget::Exhaustive,
        };
        let tuning: u32 = budget.schedule().iter().map(|r| r.trials as u32).sum();
        let validation = budget.holdout_top_n() as u32;
        Some((tuning, validation, 1))
    }
}

impl Aggregate for EvaluationRun {
    type Event = EvaluationRunEvent;
    type Command = EvaluationRunCommand;
    type Error = EvaluationRunError;

    fn aggregate_type() -> &'static str {
        "evaluation_run"
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            Self::Event::RunRequested(_) => {}
            Self::Event::VariantPrepared(e) => {
                self.prepared_labels.insert(e.variant_label.clone());
                self.status = EvaluationRunStatus::Running {
                    variants_completed: 0,
                };
            }
            Self::Event::VariantScored(e) => {
                self.scored_keys.insert(ScoredVariantKey {
                    variant_label: e.variant_label.clone(),
                    options: e.options.clone(),
                    split: e.split,
                });
                self.status = EvaluationRunStatus::Running {
                    variants_completed: self.scored_keys.len() as u32,
                };
            }
            Self::Event::TrialProposed(e) => {
                self.proposed_trials.insert(e.trial_id);
            }
            Self::Event::RungAdvanced(_) => {}
            Self::Event::ChampionSelected(e) => {
                self.champion_trial_id = Some(e.trial_id);
            }
            Self::Event::RunCompleted(_) => {
                self.status = EvaluationRunStatus::Completed;
            }
            Self::Event::RunFailed(e) => {
                self.status = EvaluationRunStatus::Failed {
                    reason: e.reason.clone(),
                };
            }
        }
    }

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            Self::Command::RequestRun(cmd) => match state {
                None => Ok(vec![Self::Event::RunRequested(RunRequested {
                    run_id: cmd.run_id,
                    dataset_id: cmd.dataset_id,
                    pipeline_configuration_id: cmd.pipeline_configuration_id,
                    document_id: cmd.document_id,
                    document_version: cmd.document_version,
                    variants: cmd.variants,
                    options: cmd.options,
                    autotune_request: cmd.autotune_request,
                    optimization: cmd.optimization,
                    scoring_policy: cmd.scoring_policy,
                    occurred_at: cmd.occurred_at,
                })]),
                Some(run) if run.status.is_terminal() => {
                    if matches!(run.status, EvaluationRunStatus::Completed) {
                        Ok(vec![])
                    } else {
                        Ok(vec![Self::Event::RunRequested(RunRequested {
                            run_id: cmd.run_id,
                            dataset_id: cmd.dataset_id,
                            pipeline_configuration_id: cmd.pipeline_configuration_id,
                            document_id: cmd.document_id,
                            document_version: cmd.document_version,
                            variants: cmd.variants,
                            options: cmd.options,
                            autotune_request: cmd.autotune_request,
                            optimization: cmd.optimization,
                            scoring_policy: cmd.scoring_policy,
                            occurred_at: cmd.occurred_at,
                        })])
                    }
                }
                Some(_) => Err(EvaluationRunError::AlreadyExists),
            },

            Self::Command::MarkVariantPrepared(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                if run.prepared_labels.contains(&cmd.variant_label) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::VariantPrepared(VariantPrepared {
                    run_id: run.run_id,
                    variant_label: cmd.variant_label,
                    chunk_set_id: cmd.chunk_set_id,
                    embedding_set_id: cmd.embedding_set_id,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::ScoreVariant(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                let key = ScoredVariantKey {
                    variant_label: cmd.variant_label.clone(),
                    options: cmd.options.clone(),
                    split: cmd.split,
                };
                if run.scored_keys.contains(&key) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::VariantScored(VariantScored {
                    run_id: run.run_id,
                    variant_label: cmd.variant_label,
                    variant_config: cmd.variant_config,
                    options: cmd.options,
                    split: cmd.split,
                    chunk_set_id: cmd.chunk_set_id,
                    embedding_set_id: cmd.embedding_set_id,
                    metrics: cmd.metrics,
                    retrieval_traces: cmd.retrieval_traces,
                    selected: cmd.selected,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::ProposeTrial(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                if run.proposed_trials.contains(&cmd.trial_id) {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::TrialProposed(TrialProposed {
                    run_id: run.run_id,
                    trial_id: cmd.trial_id,
                    params: cmd.params,
                    rung: cmd.rung,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::AdvanceRung(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                Ok(vec![Self::Event::RungAdvanced(RungAdvanced {
                    run_id: run.run_id,
                    rung: cmd.rung,
                    surviving_trials: cmd.surviving_trials,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::SelectChampion(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;

                if run.champion_trial_id.is_some() {
                    return Ok(vec![]);
                }
                Ok(vec![Self::Event::ChampionSelected(ChampionSelected {
                    run_id: run.run_id,
                    trial_id: cmd.trial_id,
                    holdout_metrics: cmd.holdout_metrics,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::CompleteRun(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                match &run.status {
                    EvaluationRunStatus::Completed => return Ok(vec![]),
                    EvaluationRunStatus::Failed { .. } => {
                        return Err(EvaluationRunError::AlreadyFailed)
                    }
                    _ => {}
                }
                if let Some((expected_tuning, expected_validation, expected_holdout)) =
                    run.expected_optimization_counts()
                {
                    let mut tuning = 0u32;
                    let mut validation = 0u32;
                    let mut holdout = 0u32;
                    for k in &run.scored_keys {
                        match k.split {
                            EvaluationResultSplit::Tuning => tuning += 1,
                            EvaluationResultSplit::Validation => validation += 1,
                            EvaluationResultSplit::Holdout => holdout += 1,
                            EvaluationResultSplit::Full => {}
                        }
                    }
                    if tuning < expected_tuning
                        || validation < expected_validation
                        || holdout < expected_holdout
                        || run.champion_trial_id.is_none()
                    {
                        return Err(EvaluationRunError::NotAllVariantsScored);
                    }
                } else if (run.scored_keys.len() as u32) < run.expected_score_count() {
                    return Err(EvaluationRunError::NotAllVariantsScored);
                }
                Ok(vec![Self::Event::RunCompleted(RunCompleted {
                    run_id: run.run_id,
                    occurred_at: cmd.occurred_at,
                })])
            }

            Self::Command::FailRun(cmd) => {
                let run = state.ok_or(EvaluationRunError::NotFound)?;
                match &run.status {
                    EvaluationRunStatus::Completed => {
                        return Err(EvaluationRunError::AlreadyCompleted)
                    }
                    EvaluationRunStatus::Failed { .. } => return Ok(vec![]),
                    _ => {}
                }
                Ok(vec![Self::Event::RunFailed(RunFailed {
                    run_id: run.run_id,
                    reason: cmd.reason,
                    occurred_at: cmd.occurred_at,
                })])
            }
        }
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        let mut state: Option<Self> = None;

        for event in events {
            match (&mut state, event) {
                (None, Self::Event::RunRequested(e)) => {
                    state = Some(Self::from_requested(e));
                }
                (Some(_), Self::Event::RunRequested(e)) => {
                    state = Some(Self::from_requested(e));
                }
                (None, _) => return None,
                (Some(run), event) => run.apply(event),
            }
        }

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::{ChunkingConfig, ChunkingVariant, EvaluationRunOptions};
    use uuid::Uuid;

    fn section_config() -> ChunkingConfig {
        use crate::shared::SectionChunkingConfig;
        ChunkingConfig::Section(SectionChunkingConfig {
            max_section_tokens: 512,
        })
    }

    fn make_request_cmd(run_id: Uuid, dataset_id: Uuid) -> EvaluationRunCommand {
        use super::super::commands::RequestRun;
        EvaluationRunCommand::RequestRun(RequestRun {
            run_id,
            dataset_id,
            pipeline_configuration_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            document_version: 1,
            variants: vec![ChunkingVariant {
                label: "section-512".to_string(),
                config: section_config(),
            }],
            options: vec![EvaluationRunOptions::default()],
            autotune_request: None,
            optimization: None,
            scoring_policy: ScoringPolicy::default(),
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })
    }

    fn make_run_requested_event(run_id: Uuid, dataset_id: Uuid) -> EvaluationRunEvent {
        EvaluationRunEvent::RunRequested(RunRequested {
            run_id,
            dataset_id,
            pipeline_configuration_id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            document_version: 1,
            variants: vec![ChunkingVariant {
                label: "section-512".to_string(),
                config: section_config(),
            }],
            options: vec![EvaluationRunOptions::default()],
            autotune_request: None,
            optimization: None,
            scoring_policy: ScoringPolicy::default(),
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })
    }

    #[test]
    fn request_run_emits_run_requested() {
        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let events =
            EvaluationRun::handle_command(None, make_request_cmd(run_id, dataset_id)).unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EvaluationRunEvent::RunRequested(_)));

        let run = EvaluationRun::from_events(&events).unwrap();
        assert_eq!(run.run_id, run_id);
        assert_eq!(run.dataset_id, dataset_id);
        assert!(matches!(run.status, EvaluationRunStatus::Pending));
    }

    #[test]
    fn non_run_requested_first_event_returns_none() {
        let run_id = Uuid::new_v4();
        let events = vec![EvaluationRunEvent::RunCompleted(RunCompleted {
            run_id,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        })];
        assert!(EvaluationRun::from_events(&events).is_none());
    }

    #[test]
    fn mark_variant_prepared_is_idempotent() {
        use super::super::commands::MarkVariantPrepared;
        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let mut events = vec![make_run_requested_event(run_id, dataset_id)];
        let run = EvaluationRun::from_events(&events).unwrap();

        let cmd = EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
            run_id,
            variant_label: "section-512".to_string(),
            chunk_set_id: Uuid::new_v4(),
            embedding_set_id: Uuid::new_v4(),
            occurred_at: "2024-01-01T00:01:00Z".into(),
        });
        let new_events = EvaluationRun::handle_command(Some(&run), cmd).unwrap();
        events.extend(new_events);

        let run = EvaluationRun::from_events(&events).unwrap();
        assert!(run.prepared_labels.contains("section-512"));

        let cmd2 = EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
            run_id,
            variant_label: "section-512".to_string(),
            chunk_set_id: Uuid::new_v4(),
            embedding_set_id: Uuid::new_v4(),
            occurred_at: "2024-01-01T00:01:30Z".into(),
        });
        let no_op = EvaluationRun::handle_command(Some(&run), cmd2).unwrap();
        assert!(no_op.is_empty());
    }

    #[test]
    fn score_variant_is_idempotent() {
        use super::super::commands::{MarkVariantPrepared, ScoreVariant};
        use crate::shared::{EvaluationMetrics, EvaluationResultSplit};

        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let mut events = vec![make_run_requested_event(run_id, dataset_id)];
        let run = EvaluationRun::from_events(&events).unwrap();
        events.extend(
            EvaluationRun::handle_command(
                Some(&run),
                EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                    run_id,
                    variant_label: "section-512".to_string(),
                    chunk_set_id: Uuid::new_v4(),
                    embedding_set_id: Uuid::new_v4(),
                    occurred_at: "2024-01-01T00:01:00Z".into(),
                }),
            )
            .unwrap(),
        );

        let run = EvaluationRun::from_events(&events).unwrap();
        let metrics = EvaluationMetrics {
            recall_mean: 0.8,
            recall_std: 0.1,
            precision_mean: 0.7,
            precision_std: 0.1,
            iou_mean: 0.6,
            iou_std: 0.05,
            precision_omega_mean: 0.75,
            precision_omega_std: 0.1,
            chunk_count: 0,
            average_chunk_tokens: 0,
            average_retrieved_tokens: 0,
            recall_ci_low: 0.0,
            recall_ci_high: 0.0,
            precision_ci_low: 0.0,
            precision_ci_high: 0.0,
            iou_ci_low: 0.0,
            iou_ci_high: 0.0,
            precision_omega_ci_low: 0.0,
            precision_omega_ci_high: 0.0,
            composite_ci_low: 0.0,
            composite_ci_high: 0.0,
            judge_score: None,
        };
        let score_cmd = ScoreVariant {
            run_id,
            variant_label: "section-512".to_string(),
            variant_config: section_config(),
            options: EvaluationRunOptions::default(),
            split: EvaluationResultSplit::Full,
            chunk_set_id: Uuid::new_v4(),
            embedding_set_id: Uuid::new_v4(),
            metrics: metrics.clone(),
            retrieval_traces: vec![],
            selected: true,
            occurred_at: "2024-01-01T00:02:00Z".into(),
        };
        events.extend(
            EvaluationRun::handle_command(
                Some(&run),
                EvaluationRunCommand::ScoreVariant(score_cmd),
            )
            .unwrap(),
        );

        let run = EvaluationRun::from_events(&events).unwrap();
        assert_eq!(run.scored_keys.len(), 1);

        let dup = EvaluationRun::handle_command(
            Some(&run),
            EvaluationRunCommand::ScoreVariant(ScoreVariant {
                run_id,
                variant_label: "section-512".to_string(),
                variant_config: section_config(),
                options: EvaluationRunOptions::default(),
                split: EvaluationResultSplit::Full,
                chunk_set_id: Uuid::new_v4(),
                embedding_set_id: Uuid::new_v4(),
                metrics,
                retrieval_traces: vec![],
                selected: true,
                occurred_at: "2024-01-01T00:02:30Z".into(),
            }),
        )
        .unwrap();
        assert!(dup.is_empty());
    }

    #[test]
    fn complete_before_all_scored_fails() {
        use super::super::commands::CompleteRun;
        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let events = vec![make_run_requested_event(run_id, dataset_id)];
        let run = EvaluationRun::from_events(&events).unwrap();

        let err = EvaluationRun::handle_command(
            Some(&run),
            EvaluationRunCommand::CompleteRun(CompleteRun {
                run_id,
                occurred_at: "2024-01-01T00:05:00Z".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, EvaluationRunError::NotAllVariantsScored));
    }

    #[test]
    fn complete_after_all_scored_succeeds_and_is_idempotent() {
        use super::super::commands::{CompleteRun, MarkVariantPrepared, ScoreVariant};
        use crate::shared::{EvaluationMetrics, EvaluationResultSplit};

        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let mut events = vec![make_run_requested_event(run_id, dataset_id)];

        let run = EvaluationRun::from_events(&events).unwrap();
        events.extend(
            EvaluationRun::handle_command(
                Some(&run),
                EvaluationRunCommand::MarkVariantPrepared(MarkVariantPrepared {
                    run_id,
                    variant_label: "section-512".to_string(),
                    chunk_set_id: Uuid::new_v4(),
                    embedding_set_id: Uuid::new_v4(),
                    occurred_at: "2024-01-01T00:01:00Z".into(),
                }),
            )
            .unwrap(),
        );

        let run = EvaluationRun::from_events(&events).unwrap();
        let metrics = EvaluationMetrics {
            recall_mean: 0.8,
            recall_std: 0.1,
            precision_mean: 0.7,
            precision_std: 0.1,
            iou_mean: 0.6,
            iou_std: 0.05,
            precision_omega_mean: 0.75,
            precision_omega_std: 0.1,
            chunk_count: 0,
            average_chunk_tokens: 0,
            average_retrieved_tokens: 0,
            recall_ci_low: 0.0,
            recall_ci_high: 0.0,
            precision_ci_low: 0.0,
            precision_ci_high: 0.0,
            iou_ci_low: 0.0,
            iou_ci_high: 0.0,
            precision_omega_ci_low: 0.0,
            precision_omega_ci_high: 0.0,
            composite_ci_low: 0.0,
            composite_ci_high: 0.0,
            judge_score: None,
        };
        events.extend(
            EvaluationRun::handle_command(
                Some(&run),
                EvaluationRunCommand::ScoreVariant(ScoreVariant {
                    run_id,
                    variant_label: "section-512".to_string(),
                    variant_config: section_config(),
                    options: EvaluationRunOptions::default(),
                    split: EvaluationResultSplit::Full,
                    chunk_set_id: Uuid::new_v4(),
                    embedding_set_id: Uuid::new_v4(),
                    metrics,
                    retrieval_traces: vec![],
                    selected: true,
                    occurred_at: "2024-01-01T00:02:00Z".into(),
                }),
            )
            .unwrap(),
        );

        let run = EvaluationRun::from_events(&events).unwrap();
        let complete_events = EvaluationRun::handle_command(
            Some(&run),
            EvaluationRunCommand::CompleteRun(CompleteRun {
                run_id,
                occurred_at: "2024-01-01T00:03:00Z".into(),
            }),
        )
        .unwrap();
        events.extend(complete_events);

        let run = EvaluationRun::from_events(&events).unwrap();
        assert!(matches!(run.status, EvaluationRunStatus::Completed));

        let no_op = EvaluationRun::handle_command(
            Some(&run),
            EvaluationRunCommand::CompleteRun(CompleteRun {
                run_id,
                occurred_at: "2024-01-01T00:04:00Z".into(),
            }),
        )
        .unwrap();
        assert!(no_op.is_empty());
    }

    #[test]
    fn fail_run_is_idempotent() {
        use super::super::commands::FailRun;
        let run_id = Uuid::new_v4();
        let dataset_id = Uuid::new_v4();
        let mut events = vec![make_run_requested_event(run_id, dataset_id)];
        let run = EvaluationRun::from_events(&events).unwrap();
        events.extend(
            EvaluationRun::handle_command(
                Some(&run),
                EvaluationRunCommand::FailRun(FailRun {
                    run_id,
                    reason: "embedding timeout".to_string(),
                    occurred_at: "2024-01-01T00:02:00Z".into(),
                }),
            )
            .unwrap(),
        );

        let run = EvaluationRun::from_events(&events).unwrap();
        assert!(matches!(run.status, EvaluationRunStatus::Failed { .. }));

        let no_op = EvaluationRun::handle_command(
            Some(&run),
            EvaluationRunCommand::FailRun(FailRun {
                run_id,
                reason: "still failing".to_string(),
                occurred_at: "2024-01-01T00:03:00Z".into(),
            }),
        )
        .unwrap();
        assert!(no_op.is_empty());
    }

    #[test]
    fn compute_id_canonical_json_is_locked() {
        let dataset_id = uuid::uuid!("00000000-0000-0000-0000-000000000001");
        let pipeline_id = uuid::uuid!("00000000-0000-0000-0000-000000000002");
        let variants = vec![ChunkingVariant {
            label: "section-512".to_string(),
            config: section_config(),
        }];
        let options = vec![EvaluationRunOptions::default()];

        let expected_json = r#"["00000000-0000-0000-0000-000000000001","00000000-0000-0000-0000-000000000002",[{"label":"section-512","config":{"section":{"max_section_tokens":512}}}],[{"top_k":5,"min_score_milli":0}],null,null]"#;
        let actual_json = serde_json::to_string(&(
            dataset_id,
            pipeline_id,
            &variants,
            &options,
            Option::<&crate::shared::EvaluationAutotuneRequest>::None,
            Option::<&OptimizationConfig>::None,
        ))
        .unwrap();
        assert_eq!(
            actual_json, expected_json,
            "compute_id JSON payload changed — run IDs will fork"
        );

        let id =
            EvaluationRun::compute_id(dataset_id, pipeline_id, &variants, &options, None, None);
        let expected_id = uuid::Uuid::new_v5(&EVAL_RUN_NAMESPACE, expected_json.as_bytes());
        assert_eq!(id, expected_id);
    }

    #[test]
    fn compute_id_is_deterministic() {
        let dataset_id = Uuid::new_v4();
        let pipeline_id = Uuid::new_v4();
        let variants = vec![ChunkingVariant {
            label: "section-512".to_string(),
            config: section_config(),
        }];
        let options = vec![EvaluationRunOptions::default()];

        let id1 =
            EvaluationRun::compute_id(dataset_id, pipeline_id, &variants, &options, None, None);
        let id2 =
            EvaluationRun::compute_id(dataset_id, pipeline_id, &variants, &options, None, None);
        assert_eq!(id1, id2);
    }

    #[test]
    fn compute_id_differs_for_different_params() {
        let dataset_id = Uuid::new_v4();
        let pipeline_id = Uuid::new_v4();
        let variants_a = vec![ChunkingVariant {
            label: "section-512".to_string(),
            config: section_config(),
        }];
        let variants_b = vec![ChunkingVariant {
            label: "section-256".to_string(),
            config: ChunkingConfig::Section(crate::shared::SectionChunkingConfig {
                max_section_tokens: 256,
            }),
        }];
        let options = vec![EvaluationRunOptions::default()];

        let id_a =
            EvaluationRun::compute_id(dataset_id, pipeline_id, &variants_a, &options, None, None);
        let id_b =
            EvaluationRun::compute_id(dataset_id, pipeline_id, &variants_b, &options, None, None);
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn compute_id_differs_for_different_optimization_configs() {
        use crate::shared::{OptimizationBudget, OptimizationConfig, OptimizationScope};

        let dataset_id = Uuid::new_v4();
        let pipeline_id = Uuid::new_v4();
        let variants: Vec<ChunkingVariant> = Vec::new();
        let options: Vec<EvaluationRunOptions> = Vec::new();

        let cfg_a = OptimizationConfig {
            budget: OptimizationBudget::Quick,
            scope: OptimizationScope::Both,
            judges_enabled: false,
            seed: Some(1),
        };
        let cfg_b = OptimizationConfig {
            budget: OptimizationBudget::Thorough,
            scope: OptimizationScope::Both,
            judges_enabled: false,
            seed: Some(1),
        };
        let cfg_c = OptimizationConfig {
            budget: OptimizationBudget::Quick,
            scope: OptimizationScope::Both,
            judges_enabled: false,
            seed: Some(2),
        };

        let id_a = EvaluationRun::compute_id(
            dataset_id,
            pipeline_id,
            &variants,
            &options,
            None,
            Some(&cfg_a),
        );
        let id_b = EvaluationRun::compute_id(
            dataset_id,
            pipeline_id,
            &variants,
            &options,
            None,
            Some(&cfg_b),
        );
        let id_c = EvaluationRun::compute_id(
            dataset_id,
            pipeline_id,
            &variants,
            &options,
            None,
            Some(&cfg_c),
        );
        assert_ne!(id_a, id_b, "different budgets must produce different ids");
        assert_ne!(id_a, id_c, "different seeds must produce different ids");
    }
}
