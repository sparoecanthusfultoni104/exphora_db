use crate::models::{val_to_str, JsonRecord};
use crate::parser::{export_to_json, export_to_markdown, export_to_pdf, export_to_xlsx};

// ── CSV export (not in parser.rs — implemented here directly) ─────────────────

fn export_to_csv_inline(
    records: &[&JsonRecord],
    columns: &[String],
    path: &std::path::Path,
) -> Result<(), String> {
    let mut wtr = csv::WriterBuilder::new()
        .from_path(path)
        .map_err(|e| format!("Error creando archivo CSV: {e}"))?;

    // Header row
    wtr.write_record(columns)
        .map_err(|e| format!("Error escribiendo cabecera CSV: {e}"))?;

    // Data rows
    for rec in records {
        let obj = rec.as_object();
        let row: Vec<String> = columns
            .iter()
            .map(|col| {
                obj.and_then(|o| o.get(col))
                    .map(|v| val_to_str(v))
                    .unwrap_or_default()
            })
            .collect();
        wtr.write_record(&row)
            .map_err(|e| format!("Error escribiendo fila CSV: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("Error guardando CSV: {e}"))
}

// ── Command ───────────────────────────────────────────────────────────────────

/// Export records to a file at `save_path`.
/// `format` must be one of: "csv" | "json" | "xlsx" | "markdown" | "pdf"
/// `dataset_name` is used as the PDF title.
#[tauri::command]
pub fn export_format(
    records: Vec<JsonRecord>,
    columns: Vec<String>,
    format: String,
    save_path: String,
    dataset_name: String,
) -> Result<(), String> {
    let path = std::path::Path::new(&save_path);
    let refs: Vec<&JsonRecord> = records.iter().collect();

    match format.as_str() {
        "csv" => export_to_csv_inline(&refs, &columns, path),
        "json" => export_to_json(&refs, &columns, path),
        "xlsx" => export_to_xlsx(&refs, &columns, path),
        "markdown" => export_to_markdown(&refs, &columns, path),
        "pdf" => export_to_pdf(&refs, &columns, path, &dataset_name),
        other => Err(format!("Formato de exportación desconocido: '{other}'")),
    }
}
