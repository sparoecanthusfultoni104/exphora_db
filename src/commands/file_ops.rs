use crate::commands::AppInfoState;
use crate::models::JsonRecord;
use crate::parser::{infer_schema, parse_csv, parse_ndjson, parse_sqlite, parse_xml};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

// ── Shared data types returned to the frontend ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedTab {
    pub id: String,
    pub name: String,
    pub columns: Vec<String>,
    pub records: Vec<JsonRecord>,
    pub total_rows: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub build_date: String,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Returns app version and build date for display in the frontend title bar.
#[tauri::command]
pub fn get_app_info(state: State<AppInfoState>) -> AppInfo {
    AppInfo {
        version: state.version.clone(),
        build_date: state.build_date.clone(),
    }
}

/// Opens a native file picker and returns the selected path, or None if cancelled.
/// Supported extensions: json, csv, ndjson, jsonl, xml, db, sqlite, sqlite3.
#[tauri::command]
pub async fn open_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let result = app
        .dialog()
        .file()
        .add_filter(
            "Supported files",
            &[
                "json", "csv", "ndjson", "jsonl", "xml", "db", "sqlite", "sqlite3",
            ],
        )
        .blocking_pick_file();

    Ok(result.map(|p| p.to_string()))
}

/// Loads a file from `path`, parsing it according to its extension.
/// Returns one `LoadedTab` per dataset (SQLite may return multiple tabs).
#[tauri::command]
pub fn load_file(path: String) -> Result<Vec<LoadedTab>, String> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dataset")
        .to_string();

    match ext.as_str() {
        "db" | "sqlite" | "sqlite3" => {
            let tables = parse_sqlite(p)?;
            let tabs = tables
                .into_iter()
                .map(|(table_name, records)| {
                    let schema = infer_schema(&records);
                    let columns: Vec<String> =
                        schema.fields.iter().map(|f| f.name.clone()).collect();
                    let total_rows = records.len();
                    LoadedTab {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: format!("{stem}/{table_name}"),
                        columns,
                        records,
                        total_rows,
                    }
                })
                .collect();
            Ok(tabs)
        }
        _ => {
            let data = std::fs::read(&path).map_err(|e| format!("Error leyendo archivo: {e}"))?;

            let records = match ext.as_str() {
                "csv" => parse_csv(&data)?,
                "xml" => parse_xml(&data)?,
                "ndjson" | "jsonl" => parse_ndjson(&data)?,
                "json" | _ => {
                    // Try JSON array first, fall back to NDJSON
                    serde_json::from_slice::<Vec<JsonRecord>>(&data)
                        .or_else(|_| parse_ndjson(&data))
                        .map_err(|e| format!("Error parseando JSON: {e}"))?
                }
            };

            let schema = infer_schema(&records);
            let columns: Vec<String> = schema.fields.iter().map(|f| f.name.clone()).collect();
            let total_rows = records.len();

            Ok(vec![LoadedTab {
                id: uuid::Uuid::new_v4().to_string(),
                name: stem,
                columns,
                records,
                total_rows,
            }])
        }
    }
}

/// Returns (sorted by freq desc) unique values for `col` within `filtered_indices`.
/// This helper is also used by filter_ops; kept here so `LoadedTab` stays in scope.
pub fn compute_unique_values_impl(
    col: &str,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    max_unique: usize,
) -> Vec<(String, usize)> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    for &idx in filtered_indices {
        let cell = records
            .get(idx)
            .and_then(|r| r.as_object())
            .and_then(|o| o.get(col))
            .map(|v| crate::models::val_to_str(v))
            .unwrap_or_default();
        let key = if cell.is_empty() {
            "null".to_string()
        } else {
            cell
        };
        *freq.entry(key).or_insert(0) += 1;
    }
    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    pairs.truncate(max_unique);
    pairs
}
