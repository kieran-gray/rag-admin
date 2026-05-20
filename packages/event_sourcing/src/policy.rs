use super::envelope::EventEnvelope;
use super::job_queue::NewJob;

pub struct PolicyContext<'a, A, E> {
    pub envelope: &'a EventEnvelope<E>,
    pub state: &'a A,
}

impl<'a, A, E> PolicyContext<'a, A, E> {
    pub fn new(envelope: &'a EventEnvelope<E>, state: &'a A) -> Self {
        Self { envelope, state }
    }
}

pub type PolicyFn<E, A, R> = fn(&E, &PolicyContext<'_, A, E>) -> Vec<NewJob<R>>;

pub trait HasPolicies<A: 'static, R: 'static>: Sized + 'static {
    fn policies() -> &'static [PolicyFn<Self, A, R>] {
        &[]
    }

    fn apply_policies(&self, ctx: &PolicyContext<'_, A, Self>) -> Vec<NewJob<R>> {
        Self::policies().iter().flat_map(|f| f(self, ctx)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{EventEnvelope, EventMetadata};
    use crate::job_queue::{IdempotencyKey, NewJob};
    use uuid::Uuid;

    struct StateA;
    #[derive(Clone)]
    struct EvA;
    impl HasPolicies<StateA, ()> for EvA {}

    struct StateB;
    #[derive(Clone)]
    struct EvB;

    fn one_job(_ev: &EvB, ctx: &PolicyContext<'_, StateB, EvB>) -> Vec<NewJob<()>> {
        vec![NewJob {
            partition_key: ctx.envelope.metadata.stream_id,
            job_type: "j",
            idempotency_key: IdempotencyKey::new(ctx.envelope.metadata.stream_id, 0, "j"),
            payload: (),
        }]
    }

    impl HasPolicies<StateB, ()> for EvB {
        fn policies() -> &'static [PolicyFn<Self, StateB, ()>] {
            &[one_job]
        }
    }

    fn make_env<E: Clone>(event: E) -> EventEnvelope<E> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id: Uuid::nil(),
                aggregate_type: "t".into(),
                sequence: 1,
                log_position: 1,
                event_type: "T".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn default_implementation_produces_no_jobs() {
        let env = make_env(EvA);
        let ctx = PolicyContext::new(&env, &StateA);
        assert!(EvA.apply_policies(&ctx).is_empty());
    }

    #[test]
    fn registered_policy_is_invoked_and_jobs_collected() {
        let env = make_env(EvB);
        let ctx = PolicyContext::new(&env, &StateB);
        let jobs = EvB.apply_policies(&ctx);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, "j");
    }
}
