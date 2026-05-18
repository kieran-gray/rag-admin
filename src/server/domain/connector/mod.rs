pub mod aggregate;
pub mod commands;
pub mod config;
pub mod events;
pub mod exceptions;
pub mod projector;
pub mod read_model;
pub mod repository;

pub use aggregate::Connector;
pub use commands::{
    ConnectorCommand, RegisterConnector, RenameConnector, UnregisterConnector,
    UpdateConnectorConfig,
};
pub use config::{ConnectorConfig, ConnectorKind, SitemapConfig};
pub use events::{
    ConnectorConfigUpdated, ConnectorEvent, ConnectorRegistered, ConnectorRenamed,
    ConnectorUnregistered,
};
pub use exceptions::ConnectorError;
pub use projector::ConnectorProjector;
pub use read_model::ConnectorReadModel;
pub use repository::{ConnectorRepository, ConnectorRepositoryError};
