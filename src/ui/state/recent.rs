const LS_KEY: &str = "rag-admin.recent";
const CAP: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentLocation {
    pub path: String,
    pub title: String,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn read_paths() -> Vec<String> {
    storage()
        .and_then(|s| s.get_item(LS_KEY).ok().flatten())
        .map(|raw| {
            raw.lines()
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn recent_locations() -> Vec<RecentLocation> {
    read_paths()
        .into_iter()
        .map(|path| RecentLocation {
            title: title_for_path(&path),
            path,
        })
        .collect()
}

pub fn record_location(path: &str) {
    if path.is_empty() || path == "/" || path.contains("/by-id/") {
        return;
    }
    let Some(store) = storage() else {
        return;
    };
    let mut paths = read_paths();
    paths.retain(|p| p != path);
    paths.insert(0, path.to_string());
    paths.truncate(CAP);
    _ = store.set_item(LS_KEY, &paths.join("\n"));
}

pub fn title_for_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed {
        "" => "Home".into(),
        "/documents" => "Documents".into(),
        "/workflows/ingest" => "Ingest".into(),
        "/workflows/evaluate" => "Evaluate".into(),
        "/workflows/runs" => "Runs".into(),
        "/playground/embed" => "Playground · Embed".into(),
        "/playground/retrieve" => "Playground · Retrieve".into(),
        "/playground/chat" => "Playground · Chat".into(),
        "/configuration/catalog" => "Catalog".into(),
        "/configuration/profiles" => "Profiles".into(),
        "/configuration/chunking" => "Chunking".into(),
        "/configuration/connectors" => "Connectors".into(),
        "/artifacts/chunk-sets" => "Chunk sets".into(),
        "/artifacts/maps" => "Maps".into(),
        "/artifacts/datasets" => "Datasets".into(),
        other => prefix_title(other),
    }
}

fn prefix_title(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/workflows/runs/") {
        return format!("Run {}", short(rest));
    }
    if let Some(rest) = path.strip_prefix("/artifacts/datasets/") {
        return format!("Dataset {}", short(rest));
    }
    if let Some(rest) = path.strip_prefix("/documents/") {
        return document_title(rest);
    }
    if let Some(rest) = path.strip_prefix("/evaluate/") {
        return format!("Evaluate · {}", document_label(rest));
    }
    path.to_string()
}

fn document_title(rest: &str) -> String {
    // rest = "{doc_type}/{source_ref}" optionally followed by "/ingest" or "/map"
    let Some((_doc_type, tail)) = rest.split_once('/') else {
        return "Documents".into();
    };
    let (source_ref, suffix) = match tail.rsplit_once('/') {
        Some((sr, s)) if s == "ingest" || s == "map" => (sr, Some(s)),
        _ => (tail, None),
    };
    let label = shorten(&percent_decode(source_ref), 28);
    match suffix {
        Some("ingest") => format!("Ingest · {label}"),
        Some("map") => format!("Map · {label}"),
        _ => label,
    }
}

fn document_label(rest: &str) -> String {
    let source_ref = rest.split_once('/').map(|(_, sr)| sr).unwrap_or(rest);
    shorten(&percent_decode(source_ref), 24)
}

fn short(s: &str) -> String {
    s.chars().take(8).collect()
}

fn shorten(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(n).collect();
        format!("{prefix}…")
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'%' {
            if let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            ) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
