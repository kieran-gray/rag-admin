use uuid::Uuid;

use crate::shared::contracts::{IndexingDto, QueryHit};
use crate::ui::components::primitives::Status;

pub fn short_hash(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

pub fn source_link(doc_id: Uuid, char_start: Option<u32>, char_end: Option<u32>) -> String {
    match (char_start, char_end) {
        (Some(start), Some(end)) => {
            format!("/documents/by-id/{doc_id}?tab=source&ref_start={start}&ref_end={end}")
        }
        _ => format!("/documents/by-id/{doc_id}?tab=source"),
    }
}

pub fn hit_source_link(hit: &QueryHit) -> Option<String> {
    hit.document_id
        .map(|doc_id| source_link(doc_id, hit.char_start, hit.char_end))
}

pub fn hit_display_title(hit: &QueryHit) -> String {
    hit.document_title
        .clone()
        .or_else(|| hit.source_ref_key.clone())
        .unwrap_or_else(|| "Unknown source".to_string())
}

pub fn document_type_label(doc_type: &str) -> &'static str {
    match doc_type {
        "Markdown" => "Markdown",
        "PlainText" => "Plain text",
        "WebPage" => "Web page",
        "Pdf" => "PDF",
        _ => "Document",
    }
}

pub fn format_when(ts: &str) -> String {
    let Some((date, time)) = ts.split_once('T') else {
        return ts.to_string();
    };
    let mut parts = date.split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return ts.to_string();
    };
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return ts.to_string();
    };
    let hm = time.get(..5).unwrap_or("");
    format!("{} {day}, {year} · {hm}", month_short(month))
}

pub fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "?",
    }
}

#[derive(Debug, Clone)]
pub struct IndexingSummary {
    pub status: Status,
    pub label: String,
}

pub fn indexing_summary(indexings: &[IndexingDto], latest_version: Option<u32>) -> IndexingSummary {
    let live: Vec<&IndexingDto> = indexings.iter().filter(|i| !i.removed).collect();
    if live.is_empty() {
        return IndexingSummary {
            status: Status::Info,
            label: "Registered".into(),
        };
    }
    if live.iter().any(|i| i.status.contains("Failed")) {
        return IndexingSummary {
            status: Status::Fail,
            label: "Failed".into(),
        };
    }

    let indexed: Vec<&IndexingDto> = live
        .iter()
        .copied()
        .filter(|i| i.status.contains("Indexed"))
        .collect();

    if !indexed.is_empty() {
        let stale_against =
            latest_version.filter(|v| indexed.iter().any(|i| i.document_version < *v));
        if let Some(v) = stale_against {
            return IndexingSummary {
                status: Status::Pending,
                label: format!("Indexed · stale (v{v})"),
            };
        }
        return if indexed.len() == 1 {
            IndexingSummary {
                status: Status::Ok,
                label: "Indexed".into(),
            }
        } else {
            IndexingSummary {
                status: Status::Ok,
                label: format!("Indexed × {}", indexed.len()),
            }
        };
    }

    let Some(most_advanced) = live.iter().copied().max_by_key(|ix| indexing_milestone(ix)) else {
        return IndexingSummary {
            status: Status::Info,
            label: "Registered".into(),
        };
    };
    match (most_advanced.chunk_set_id, most_advanced.embedding_set_id) {
        (None, _) => IndexingSummary {
            status: Status::Pending,
            label: "Chunking…".into(),
        },
        (Some(_), None) => IndexingSummary {
            status: Status::Info,
            label: "Chunked".into(),
        },
        (Some(_), Some(_)) => IndexingSummary {
            status: Status::Info,
            label: "Embedded".into(),
        },
    }
}

pub fn indexing_milestone(ix: &IndexingDto) -> u8 {
    if ix.status.contains("Indexed") {
        4
    } else if ix.embedding_set_id.is_some() {
        3
    } else if ix.chunk_set_id.is_some() {
        2
    } else {
        1
    }
}
