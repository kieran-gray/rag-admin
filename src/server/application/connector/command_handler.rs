use std::sync::Arc;

use crate::server::application::ports::{Clock, IdGenerator};
use crate::server::application::AppError;
use crate::server::domain::connector::{
    Connector, ConnectorCommand, ConnectorConfig, RegisterConnector, RenameConnector,
    SetConnectorDefaults, SitemapConfig, UnregisterConnector, UpdateConnectorConfig,
};
use crate::shared::contracts::connector::{ConnectorCommandDto, ConnectorConfigDto, ConnectorDto};
use event_sourcing::CommandProcessor;
use uuid::Uuid;

use super::query_service::ConnectorQueryService;

pub struct ConnectorCommandHandler {
    processor: Arc<CommandProcessor<Connector>>,
    query_service: Arc<ConnectorQueryService>,
    clock: Arc<dyn Clock>,
    id_generator: Arc<dyn IdGenerator>,
}

impl ConnectorCommandHandler {
    pub fn new(
        processor: Arc<CommandProcessor<Connector>>,
        query_service: Arc<ConnectorQueryService>,
        clock: Arc<dyn Clock>,
        id_generator: Arc<dyn IdGenerator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            processor,
            query_service,
            clock,
            id_generator,
        })
    }

    pub async fn handle_dto(
        &self,
        dto: ConnectorCommandDto,
    ) -> Result<Option<ConnectorDto>, AppError> {
        match dto {
            ConnectorCommandDto::RegisterConnector(d) => {
                let connector_id = self.id_generator.new_uuid();
                self.processor
                    .handle(
                        connector_id,
                        ConnectorCommand::RegisterConnector(RegisterConnector {
                            connector_id,
                            name: d.name,
                            config: config_dto_to_domain(d.config),
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                Ok(self.query_service.get(connector_id).await?)
            }
            ConnectorCommandDto::RenameConnector(d) => {
                self.processor
                    .handle(
                        d.connector_id,
                        ConnectorCommand::RenameConnector(RenameConnector {
                            connector_id: d.connector_id,
                            name: d.name,
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                Ok(self.query_service.get(d.connector_id).await?)
            }
            ConnectorCommandDto::UpdateConnectorConfig(d) => {
                self.processor
                    .handle(
                        d.connector_id,
                        ConnectorCommand::UpdateConnectorConfig(UpdateConnectorConfig {
                            connector_id: d.connector_id,
                            config: config_dto_to_domain(d.config),
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                Ok(self.query_service.get(d.connector_id).await?)
            }
            ConnectorCommandDto::UnregisterConnector(d) => {
                self.unregister(d.connector_id).await?;
                Ok(None)
            }
            ConnectorCommandDto::SetConnectorDefaults(d) => {
                self.processor
                    .handle(
                        d.connector_id,
                        ConnectorCommand::SetConnectorDefaults(SetConnectorDefaults {
                            connector_id: d.connector_id,
                            index_profile_id: d.index_profile_id,
                            chunking_configuration_id: d.chunking_configuration_id,
                            retrieval_profile_id: d.retrieval_profile_id,
                            occurred_at: self.clock.now(),
                        }),
                    )
                    .await?;
                Ok(self.query_service.get(d.connector_id).await?)
            }
        }
    }

    pub async fn unregister(&self, connector_id: Uuid) -> Result<(), AppError> {
        self.processor
            .handle(
                connector_id,
                ConnectorCommand::UnregisterConnector(UnregisterConnector {
                    connector_id,
                    occurred_at: self.clock.now(),
                }),
            )
            .await?;
        Ok(())
    }
}

fn config_dto_to_domain(dto: ConnectorConfigDto) -> ConnectorConfig {
    match dto {
        ConnectorConfigDto::Sitemap(c) => ConnectorConfig::Sitemap(SitemapConfig {
            url: c.url,
            include_patterns: c.include_patterns,
            exclude_patterns: c.exclude_patterns,
        }),
    }
}
