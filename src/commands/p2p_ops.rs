use crate::commands::file_ops::LoadedTab;
use crate::models::JsonRecord;
use crate::p2p::Command;
use crate::parser::infer_schema;
use tauri::State;
use tokio::sync::{mpsc, oneshot};

/// Managed state that holds the P2P command channel sender.
pub struct P2pState {
    pub cmd_tx: mpsc::Sender<Command>,
}

// ── Helper: extract dataset name from share link ──────────────────────────────
// Share links have format: exphora:<name>:<rest>
// We extract <name> for the tab label.
fn name_from_link(link: &str) -> String {
    let without_scheme = link.strip_prefix("exphora:").unwrap_or(link);
    without_scheme
        .split(':')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("p2p_dataset")
        .to_string()
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Compress, shard and serve a dataset via P2P.
/// Returns the "exphora:..." share link string that peers can use to fetch.
#[tauri::command]
pub async fn p2p_share(
    name: String,
    records: Vec<JsonRecord>,
    port: u16,
    state: State<'_, P2pState>,
) -> Result<String, String> {
    let json_bytes =
        serde_json::to_vec(&records).map_err(|e| format!("Error serializando records: {e}"))?;

    let (resp_tx, resp_rx) = oneshot::channel();
    state
        .cmd_tx
        .send(Command::ShareDataset {
            name,
            json_bytes,
            port,
            resp: resp_tx,
        })
        .await
        .map_err(|_| "P2P runtime not running".to_string())?;

    resp_rx
        .await
        .map_err(|_| "P2P response channel dropped".to_string())?
}

/// Fetch a dataset from a peer using an "exphora:..." share link.
/// Fully constructs a LoadedTab (schema inferred, uuid assigned).
/// No todo!() — fully implemented.
#[tauri::command]
pub async fn p2p_fetch(link: String, state: State<'_, P2pState>) -> Result<LoadedTab, String> {
    let (resp_tx, resp_rx) = oneshot::channel();
    state
        .cmd_tx
        .send(Command::FetchDataset {
            link: link.clone(),
            resp: resp_tx,
        })
        .await
        .map_err(|_| "P2P runtime not running".to_string())?;

    let json_bytes: Vec<u8> = resp_rx
        .await
        .map_err(|_| "P2P response channel dropped".to_string())??;

    // Step 1: parse JSON bytes → Vec<JsonRecord>
    // Try NDJSON first (parse_ndjson handles both JSON arrays and NDJSON lines).
    let records: Vec<JsonRecord> = crate::parser::parse_ndjson(&json_bytes).or_else(|_| {
        serde_json::from_slice::<Vec<JsonRecord>>(&json_bytes)
            .map_err(|e| format!("Error parseando JSON recibido: {e}"))
    })?;

    // Step 2: infer schema to determine column list
    let schema = infer_schema(&records);
    let columns: Vec<String> = schema.fields.iter().map(|f| f.name.clone()).collect();
    let total_rows = records.len();

    // Step 3: extract dataset name from share link prefix (exphora:<name>:...)
    let name = name_from_link(&link);

    // Step 4: construct and return a fully populated LoadedTab
    Ok(LoadedTab {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        columns,
        records,
        total_rows,
    })
}
