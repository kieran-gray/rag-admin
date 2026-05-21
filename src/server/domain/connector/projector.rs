use std::sync::Arc;

use async_trait::async_trait;

use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::projector::Projector;

use super::events::ConnectorEvent;
use super::read_model::ConnectorReadModel;
use super::repository::ConnectorRepository;

pub struct ConnectorProjector {
    repository: Arc<dyn ConnectorRepository>,
}

impl ConnectorProjector {
    pub const NAME: &'static str = "connector_projector";

    pub fn new(repository: Arc<dyn ConnectorRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl Projector<ConnectorEvent> for ConnectorProjector {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn project(
        &self,
        events: &[EventEnvelope<ConnectorEvent>],
    ) -> Result<(), ProjectionError> {
        for envelope in events {
            let connector_id = envelope.metadata.stream_id;
            match &envelope.event {
                ConnectorEvent::ConnectorRegistered(e) => {
                    self.repository
                        .save(ConnectorReadModel {
                            connector_id,
                            name: e.name.clone(),
                            config: e.config.clone(),
                            deleted: false,
                            created_at: e.occurred_at.to_string(),
                            updated_at: e.occurred_at.to_string(),
                            default_index_profile_id: None,
                            default_chunking_configuration_id: None,
                            default_retrieval_profile_id: None,
                        })
                        .await?;
                }
                ConnectorEvent::ConnectorRenamed(e) => {
                    if let Some(mut m) = self.repository.load(connector_id).await? {
                        m.name = e.name.clone();
                        m.updated_at = e.occurred_at.to_string();
                        self.repository.save(m).await?;
                    }
                }
                ConnectorEvent::ConnectorConfigUpdated(e) => {
                    if let Some(mut m) = self.repository.load(connector_id).await? {
                        m.config = e.config.clone();
                        m.updated_at = e.occurred_at.to_string();
                        self.repository.save(m).await?;
                    }
                }
                ConnectorEvent::ConnectorUnregistered(e) => {
                    self.repository
                        .mark_deleted(connector_id, e.occurred_at.to_string())
                        .await?;
                }
                ConnectorEvent::ConnectorDefaultsSet(e) => {
                    if let Some(mut m) = self.repository.load(connector_id).await? {
                        m.default_index_profile_id = e.index_profile_id;
                        m.default_chunking_configuration_id = e.chunking_configuration_id;
                        m.default_retrieval_profile_id = e.retrieval_profile_id;
                        m.updated_at = e.occurred_at.to_string();
                        self.repository.save(m).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::Mutex;

    use event_sourcing::envelope::EventMetadata;
    use uuid::Uuid;

    use crate::server::domain::connector::config::{ConnectorConfig, SitemapConfig};
    use crate::server::domain::connector::events::{
        ConnectorConfigUpdated, ConnectorDefaultsSet, ConnectorRegistered, ConnectorRenamed,
        ConnectorUnregistered,
    };
    use crate::server::domain::connector::repository::ConnectorRepositoryError;

    #[derive(Default)]
    struct InMemConnectorRepo {
        records: Mutex<HashMap<Uuid, ConnectorReadModel>>,
        fail_next_load: Mutex<bool>,
    }

    impl InMemConnectorRepo {
        fn get(&self, id: Uuid) -> Option<ConnectorReadModel> {
            self.records.lock().expect("lock").get(&id).cloned()
        }
    }

    #[async_trait]
    impl ConnectorRepository for InMemConnectorRepo {
        async fn load(
            &self,
            connector_id: Uuid,
        ) -> Result<Option<ConnectorReadModel>, ConnectorRepositoryError> {
            if *self.fail_next_load.lock().expect("lock") {
                *self.fail_next_load.lock().expect("lock") = false;
                return Err(ConnectorRepositoryError::Internal("forced".into()));
            }
            Ok(self
                .records
                .lock()
                .expect("lock")
                .get(&connector_id)
                .cloned())
        }

        async fn save(&self, model: ConnectorReadModel) -> Result<(), ConnectorRepositoryError> {
            self.records
                .lock()
                .expect("lock")
                .insert(model.connector_id, model);
            Ok(())
        }

        async fn mark_deleted(
            &self,
            connector_id: Uuid,
            updated_at: String,
        ) -> Result<(), ConnectorRepositoryError> {
            if let Some(m) = self.records.lock().expect("lock").get_mut(&connector_id) {
                m.deleted = true;
                m.updated_at = updated_at;
            }
            Ok(())
        }

        async fn list_active(&self) -> Result<Vec<ConnectorReadModel>, ConnectorRepositoryError> {
            Ok(self
                .records
                .lock()
                .expect("lock")
                .values()
                .filter(|m| !m.deleted)
                .cloned()
                .collect())
        }
    }

    fn sitemap_config(url: &str) -> ConnectorConfig {
        ConnectorConfig::Sitemap(SitemapConfig {
            url: url.into(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        })
    }

    fn envelope(
        connector_id: Uuid,
        event: ConnectorEvent,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        EventEnvelope {
            event,
            metadata: EventMetadata {
                stream_id: connector_id,
                aggregate_type: "connector".into(),
                sequence: log_position,
                log_position,
                event_type: "Ev".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    fn registered(
        connector_id: Uuid,
        name: &str,
        url: &str,
        when: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        envelope(
            connector_id,
            ConnectorEvent::ConnectorRegistered(ConnectorRegistered {
                connector_id,
                name: name.into(),
                config: sitemap_config(url),
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    fn renamed(
        connector_id: Uuid,
        name: &str,
        when: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        envelope(
            connector_id,
            ConnectorEvent::ConnectorRenamed(ConnectorRenamed {
                name: name.into(),
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    fn config_updated(
        connector_id: Uuid,
        url: &str,
        when: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        envelope(
            connector_id,
            ConnectorEvent::ConnectorConfigUpdated(ConnectorConfigUpdated {
                config: sitemap_config(url),
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    fn unregistered(
        connector_id: Uuid,
        when: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        envelope(
            connector_id,
            ConnectorEvent::ConnectorUnregistered(ConnectorUnregistered {
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    fn defaults_set(
        connector_id: Uuid,
        index_profile: Option<Uuid>,
        chunking: Option<Uuid>,
        retrieval_profile: Option<Uuid>,
        when: &str,
        log_position: i64,
    ) -> EventEnvelope<ConnectorEvent> {
        envelope(
            connector_id,
            ConnectorEvent::ConnectorDefaultsSet(ConnectorDefaultsSet {
                index_profile_id: index_profile,
                chunking_configuration_id: chunking,
                retrieval_profile_id: retrieval_profile,
                occurred_at: when.into(),
            }),
            log_position,
        )
    }

    #[tokio::test]
    async fn registered_creates_read_model_with_initial_fields() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[registered(
                id,
                "docs",
                "https://example.com/sitemap.xml",
                "2024-01-01T00:00:00Z",
                1,
            )])
            .await
            .expect("project");

        let m = repo.get(id).expect("present");
        assert_eq!(m.connector_id, id);
        assert_eq!(m.name, "docs");
        assert_eq!(m.created_at, "2024-01-01T00:00:00Z");
        assert_eq!(m.updated_at, "2024-01-01T00:00:00Z");
        assert!(!m.deleted);
        assert!(m.default_index_profile_id.is_none());
        assert!(m.default_chunking_configuration_id.is_none());
        assert!(m.default_retrieval_profile_id.is_none());
        assert!(matches!(m.config, ConnectorConfig::Sitemap(_)));
    }

    #[tokio::test]
    async fn renamed_updates_name_and_updated_at_only() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "old", "https://x", "2024-01-01T00:00:00Z", 1),
                renamed(id, "new", "2024-02-01T00:00:00Z", 2),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        assert_eq!(m.name, "new");
        assert_eq!(m.created_at, "2024-01-01T00:00:00Z");
        assert_eq!(m.updated_at, "2024-02-01T00:00:00Z");
    }

    #[tokio::test]
    async fn config_updated_replaces_config_and_bumps_updated_at() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "docs", "https://x", "2024-01-01T00:00:00Z", 1),
                config_updated(id, "https://y", "2024-02-01T00:00:00Z", 2),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        let ConnectorConfig::Sitemap(c) = &m.config;
        assert_eq!(c.url, "https://y");
        assert_eq!(m.updated_at, "2024-02-01T00:00:00Z");
        assert_eq!(m.name, "docs", "rename and config-update are independent");
    }

    #[tokio::test]
    async fn unregistered_marks_record_deleted_with_new_updated_at() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "docs", "https://x", "2024-01-01T00:00:00Z", 1),
                unregistered(id, "2024-03-01T00:00:00Z", 2),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        assert!(m.deleted);
        assert_eq!(m.updated_at, "2024-03-01T00:00:00Z");
    }

    #[tokio::test]
    async fn defaults_set_writes_all_optional_fields() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();
        let index_profile = Uuid::new_v4();
        let chunking = Uuid::new_v4();
        let retrieval_profile = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "docs", "https://x", "2024-01-01T00:00:00Z", 1),
                defaults_set(
                    id,
                    Some(index_profile),
                    Some(chunking),
                    Some(retrieval_profile),
                    "2024-04-01T00:00:00Z",
                    2,
                ),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        assert_eq!(m.default_index_profile_id, Some(index_profile));
        assert_eq!(m.default_chunking_configuration_id, Some(chunking));
        assert_eq!(m.default_retrieval_profile_id, Some(retrieval_profile));
        assert_eq!(m.updated_at, "2024-04-01T00:00:00Z");
    }

    #[tokio::test]
    async fn defaults_set_can_clear_previously_assigned_ids() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "docs", "https://x", "2024-01-01T00:00:00Z", 1),
                defaults_set(
                    id,
                    Some(Uuid::new_v4()),
                    Some(Uuid::new_v4()),
                    Some(Uuid::new_v4()),
                    "2024-04-01T00:00:00Z",
                    2,
                ),
                defaults_set(id, None, None, None, "2024-05-01T00:00:00Z", 3),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        assert!(m.default_index_profile_id.is_none());
        assert!(m.default_chunking_configuration_id.is_none());
        assert!(m.default_retrieval_profile_id.is_none());
        assert_eq!(m.updated_at, "2024-05-01T00:00:00Z");
    }

    #[tokio::test]
    async fn rename_for_unknown_connector_is_silent_no_op() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[renamed(id, "new", "2024-01-01T00:00:00Z", 1)])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn config_update_for_unknown_connector_is_silent_no_op() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[config_updated(id, "https://x", "2024-01-01T00:00:00Z", 1)])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn defaults_set_for_unknown_connector_is_silent_no_op() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[defaults_set(
                id,
                None,
                None,
                None,
                "2024-01-01T00:00:00Z",
                1,
            )])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn unregister_for_unknown_connector_does_not_create_record() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[unregistered(id, "2024-01-01T00:00:00Z", 1)])
            .await
            .expect("project");

        assert!(repo.get(id).is_none());
    }

    #[tokio::test]
    async fn full_lifecycle_in_one_batch_lands_at_deleted_with_latest_metadata() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();
        let index_profile = Uuid::new_v4();

        projector
            .project(&[
                registered(id, "old", "https://x", "2024-01-01T00:00:00Z", 1),
                renamed(id, "new", "2024-02-01T00:00:00Z", 2),
                config_updated(id, "https://y", "2024-03-01T00:00:00Z", 3),
                defaults_set(
                    id,
                    Some(index_profile),
                    None,
                    None,
                    "2024-04-01T00:00:00Z",
                    4,
                ),
                unregistered(id, "2024-05-01T00:00:00Z", 5),
            ])
            .await
            .expect("project");

        let m = repo.get(id).unwrap();
        assert!(m.deleted);
        assert_eq!(m.name, "new");
        let ConnectorConfig::Sitemap(c) = &m.config;
        assert_eq!(c.url, "https://y");
        assert_eq!(m.default_index_profile_id, Some(index_profile));
        assert_eq!(m.updated_at, "2024-05-01T00:00:00Z");
    }

    #[tokio::test]
    async fn multi_connector_batch_isolates_per_stream() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        projector
            .project(&[
                registered(a, "a", "https://a", "2024-01-01T00:00:00Z", 1),
                registered(b, "b", "https://b", "2024-01-01T00:00:00Z", 2),
                renamed(a, "a-renamed", "2024-01-02T00:00:00Z", 3),
                unregistered(b, "2024-01-03T00:00:00Z", 4),
            ])
            .await
            .expect("project");

        assert_eq!(repo.get(a).unwrap().name, "a-renamed");
        assert!(!repo.get(a).unwrap().deleted);
        assert!(repo.get(b).unwrap().deleted);
    }

    #[tokio::test]
    async fn repository_load_error_propagates_through_project() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo.clone());
        let id = Uuid::new_v4();

        projector
            .project(&[registered(
                id,
                "docs",
                "https://x",
                "2024-01-01T00:00:00Z",
                1,
            )])
            .await
            .expect("seed");

        *repo.fail_next_load.lock().expect("lock") = true;
        let result = projector
            .project(&[renamed(id, "new", "2024-02-01T00:00:00Z", 2)])
            .await;
        assert!(result.is_err());
        assert_eq!(repo.get(id).unwrap().name, "docs", "save not reached");
    }

    #[tokio::test]
    async fn projector_name_is_stable_constant() {
        let repo = Arc::new(InMemConnectorRepo::default());
        let projector = ConnectorProjector::new(repo);
        assert_eq!(projector.name(), ConnectorProjector::NAME);
        assert_eq!(ConnectorProjector::NAME, "connector_projector");
    }
}
