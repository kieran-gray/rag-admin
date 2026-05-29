# event_sourcing

Generic event-sourcing framework used by `search-crucible`.

Provides:

- `Aggregate` trait and `AggregateRepository`
- `EventStore` and `EventEnvelope` types
- `Projector` / `ProjectionDriver` for read-model projections
- `ProcessManager` and `EffectExecutor` for side-effect orchestration
- `EventBus` for in-process fan-out
- `JobQueue` and `IdempotencyKey` types for durable background work
- `CheckpointRepository` for projection bookmarks

This crate is storage-agnostic. Postgres-backed implementations live alongside
the consuming app, not here.
