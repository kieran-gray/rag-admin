use super::effect::PendingEffect;
use super::envelope::EventEnvelope;

pub struct PolicyContext<'a, A, E> {
    pub envelope: &'a EventEnvelope<E>,
    pub state: &'a A,
}

impl<'a, A, E> PolicyContext<'a, A, E> {
    pub fn new(envelope: &'a EventEnvelope<E>, state: &'a A) -> Self {
        Self { envelope, state }
    }
}

pub type PolicyFn<E, A, EnvE, R> = fn(&E, &PolicyContext<'_, A, EnvE>) -> Vec<PendingEffect<R>>;

pub trait HasPolicies<A: 'static, EnvE: 'static, R: 'static>: Sized + 'static {
    fn policies() -> &'static [PolicyFn<Self, A, EnvE, R>];

    fn apply_policies(&self, ctx: &PolicyContext<'_, A, EnvE>) -> Vec<PendingEffect<R>> {
        Self::policies().iter().flat_map(|f| f(self, ctx)).collect()
    }
}
