use serde::{Deserialize, Serialize};

use super::item::{LauncherAction, LauncherEntry, LauncherGroup, LauncherIcon};
use crate::server_functions::connector::list_connectors;
use crate::server_functions::evaluation::{get_evaluation_datasets_page, get_recent_runs};
use crate::server_functions::source_document::list_documents_page;
use crate::shared::contracts::{
    ConnectorDto, DatasetListItemDto, DatasetListQueryDto, DocumentListItemDto,
    DocumentListQueryDto, IndexingDto, RecentEvaluationRunDto, SourceDescriptorDto,
};
use crate::ui::components::primitives::Status;
use crate::ui::components::shell::nav_catalog::NavIcon;

const DOCUMENT_LIMIT: u32 = 6;
const CATALOG_LIMIT: u32 = 50;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EntityCatalog {
    pub documents: Vec<DocumentListItemDto>,
    pub connectors: Vec<ConnectorDto>,
    pub datasets: Vec<DatasetListItemDto>,
    pub runs: Vec<RecentEvaluationRunDto>,
}

pub async fn search_documents(term: String) -> Vec<DocumentListItemDto> {
    if term.trim().is_empty() {
        return Vec::new();
    }
    list_documents_page(DocumentListQueryDto {
        limit: DOCUMENT_LIMIT,
        search: Some(term),
        ..Default::default()
    })
    .await
    .map(|page| page.items)
    .unwrap_or_default()
}

pub async fn load_catalog() -> EntityCatalog {
    let documents = list_documents_page(DocumentListQueryDto {
        limit: CATALOG_LIMIT,
        ..Default::default()
    })
    .await
    .map(|page| page.items)
    .unwrap_or_default();
    let connectors = list_connectors().await.unwrap_or_default();
    let datasets = get_evaluation_datasets_page(DatasetListQueryDto {
        limit: CATALOG_LIMIT,
        ..Default::default()
    })
    .await
    .map(|page| page.items)
    .unwrap_or_default();
    let runs = get_recent_runs(CATALOG_LIMIT).await.unwrap_or_default();

    EntityCatalog {
        documents,
        connectors,
        datasets,
        runs,
    }
}

pub fn document_entry(doc: &DocumentListItemDto) -> LauncherEntry {
    let href = format!(
        "/documents/{}/{}",
        doc.document_type.to_lowercase(),
        urlencoding::encode(&doc.source_ref_key),
    );
    let meta = match &doc.source {
        SourceDescriptorDto::Url { host, .. } => host.clone(),
        SourceDescriptorDto::Upload => "Upload".to_string(),
    };
    let (status, label) = document_status(doc);
    LauncherEntry::new(
        LauncherIcon::Nav(NavIcon::Documents),
        doc.title.clone(),
        LauncherGroup::Documents,
        LauncherAction::Navigate(href),
    )
    .meta(meta)
    .status(status, label)
}

pub fn dataset_entry(dataset: &DatasetListItemDto) -> LauncherEntry {
    let href = format!("/evaluation/datasets/{}", dataset.dataset_id);
    let meta = dataset
        .document_title
        .clone()
        .unwrap_or_else(|| format!("{} questions", dataset.question_count));
    let (status, label) = phase_status(&dataset.status);
    LauncherEntry::new(
        LauncherIcon::Nav(NavIcon::Datasets),
        dataset.label.clone(),
        LauncherGroup::Datasets,
        LauncherAction::Navigate(href),
    )
    .meta(meta)
    .status(status, label)
}

pub fn run_entry(run: &RecentEvaluationRunDto) -> LauncherEntry {
    let href = format!("/evaluation/runs/{}", run.run_id);
    let label = run
        .document_title
        .clone()
        .unwrap_or_else(|| format!("Run {}", short_id(&run.run_id.to_string())));
    let meta = format!("{} · {} variants", run.kind.label(), run.variant_count);
    let (status, status_label) = phase_status(&run.status);
    LauncherEntry::new(
        LauncherIcon::Nav(NavIcon::Runs),
        label,
        LauncherGroup::Runs,
        LauncherAction::Navigate(href),
    )
    .meta(meta)
    .status(status, status_label)
}

pub fn connector_entry(connector: &ConnectorDto) -> LauncherEntry {
    let href = format!("/connectors/{}", connector.connector_id);
    LauncherEntry::new(
        LauncherIcon::Nav(NavIcon::Connectors),
        connector.name.clone(),
        LauncherGroup::Connectors,
        LauncherAction::Navigate(href),
    )
    .meta("Connector")
}

fn document_status(doc: &DocumentListItemDto) -> (Status, String) {
    if doc.document_id.is_none() {
        return (Status::Stale, "not ingested".to_string());
    }
    let active: Vec<&IndexingDto> = doc.indexings.iter().filter(|i| !i.removed).collect();
    if active.iter().any(|i| i.status.contains("Failed")) {
        (Status::Fail, "failed".to_string())
    } else if active.iter().any(|i| i.status.contains("Indexed")) {
        (Status::Ok, "indexed".to_string())
    } else if active.is_empty() {
        (Status::Stale, "not indexed".to_string())
    } else {
        (Status::Pending, "indexing".to_string())
    }
}

fn phase_status(status: &str) -> (Status, String) {
    let lower = status.to_lowercase();
    if lower.contains("complet") {
        (Status::Ok, "completed".to_string())
    } else if lower.contains("fail") {
        (Status::Fail, "failed".to_string())
    } else if lower.contains("cancel") {
        (Status::Stale, "cancelled".to_string())
    } else if lower.contains("run") || lower.contains("generat") || lower.contains("pend") {
        (Status::Pending, lower)
    } else {
        (Status::Neutral, lower)
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
