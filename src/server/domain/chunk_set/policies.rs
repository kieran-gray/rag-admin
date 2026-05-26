use event_sourcing::policy::HasPolicies;

use super::aggregate::ChunkSet;
use super::effects::ChunkSetEffect;
use super::events::ChunkSetEvent;

impl HasPolicies<ChunkSet, ChunkSetEffect> for ChunkSetEvent {}
