use crate::models::{val_to_str, FieldMeta, FieldType, InferredSchema, JsonRecord};
use std::collections::HashMap;

/// Parse CSV bytes into a Vec<JsonRecord> (serde_json::Value::Object per row).
///
/// Type-inference rules (same semantics as parse_json):
///   - Empty cell              → Value::Null
///   - "true" / "false" (ci)  → Value::Bool
///   - Parseable as f64        → Value::Number
///   - Everything else         → Value::String
pub fn parse_csv(data: &[u8]) -> Result<Vec<JsonRecord>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true) // tolerate ragged rows
        .from_reader(data);

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("Error leyendo cabeceras CSV: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        return Err("El archivo CSV no tiene cabeceras".to_string());
    }

    let mut records: Vec<JsonRecord> = Vec::new();

    for (row_idx, result) in rdr.records().enumerate() {
        let row = result.map_err(|e| format!("Error en fila {}: {e}", row_idx + 2))?;

        let mut obj = serde_json::Map::new();
        for (col_idx, header) in headers.iter().enumerate() {
            let raw = row.get(col_idx).unwrap_or("").trim();
            let value = infer_csv_value(raw);
            obj.insert(header.clone(), value);
        }
        records.push(serde_json::Value::Object(obj));
    }

    Ok(records)
}

/// Infer a serde_json::Value from a raw CSV cell string.
fn infer_csv_value(raw: &str) -> serde_json::Value {
    if raw.is_empty() {
        return serde_json::Value::Null;
    }
    match raw.to_lowercase().as_str() {
        "true" => return serde_json::Value::Bool(true),
        "false" => return serde_json::Value::Bool(false),
        _ => {}
    }
    if let Ok(n) = raw.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return serde_json::Value::Number(num);
        }
    }
    serde_json::Value::String(raw.to_string())
}

const MAX_SCAN_RECORDS: usize = 1000;
const MAX_DISTINCT_FOR_DROPDOWN: usize = 500;

pub fn infer_schema(records: &[JsonRecord]) -> InferredSchema {
    let total = records.len();
    if total == 0 {
        return InferredSchema {
            fields: Vec::new(),
            search_fields: Vec::new(),
            filter_fields: Vec::new(),
            bool_fields: Vec::new(),
        };
    }

    let scan_count = total.min(MAX_SCAN_RECORDS);
    let mut field_stats: HashMap<String, FieldStats> = HashMap::new();
    let mut field_order: Vec<String> = Vec::new();

    for record in records.iter().take(scan_count) {
        if let Some(obj) = record.as_object() {
            for (key, val) in obj {
                let stats = field_stats.entry(key.clone()).or_insert_with(|| {
                    field_order.push(key.clone());
                    FieldStats::default()
                });
                stats.total_seen += 1;

                if val.is_null() {
                    stats.null_count += 1;
                } else if val.is_boolean() {
                    stats.bool_count += 1;
                    let s = val_to_str(val);
                    *stats.value_freq.entry(s).or_insert(0) += 1;
                } else if val.is_number() {
                    stats.number_count += 1;
                    let s = val_to_str(val);
                    *stats.value_freq.entry(s).or_insert(0) += 1;
                } else if val.is_string() {
                    let s = val.as_str().unwrap_or("").to_string();
                    if s.is_empty() {
                        stats.null_count += 1;
                    } else {
                        stats.text_count += 1;
                        *stats.value_freq.entry(s).or_insert(0) += 1;
                    }
                } else {
                    stats.text_count += 1;
                    let s = val_to_str(val);
                    *stats.value_freq.entry(s).or_insert(0) += 1;
                }
            }
        }
    }

    let mut fields: Vec<FieldMeta> = Vec::new();

    for name in &field_order {
        let stats = &field_stats[name];
        let non_null = stats.total_seen.saturating_sub(stats.null_count);
        let non_null_ratio = non_null as f64 / scan_count as f64;
        let distinct_count = stats.value_freq.len();

        let mut field_type = if non_null == 0 {
            FieldType::Nullable
        } else if stats.bool_count >= stats.text_count && stats.bool_count >= stats.number_count {
            FieldType::Boolean
        } else if stats.number_count >= stats.text_count {
            FieldType::Number
        } else {
            FieldType::Text
        };

        if distinct_count > MAX_DISTINCT_FOR_DROPDOWN
            && (field_type == FieldType::Text || field_type == FieldType::Number)
        {
            field_type = FieldType::FreeText;
        }

        let unique_values = if field_type == FieldType::FreeText {
            Vec::new()
        } else {
            let mut pairs: Vec<(String, usize)> = stats
                .value_freq
                .iter()
                .map(|(k, &v)| (k.clone(), v))
                .collect();
            pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            pairs.iter().take(50).map(|(k, _)| k.clone()).collect()
        };

        fields.push(FieldMeta {
            name: name.clone(),
            field_type,
            unique_values,
            non_null_ratio,
            distinct_count,
        });
    }

    let search_fields: Vec<String> = fields
        .iter()
        .filter(|f| {
            (f.field_type == FieldType::Text || f.field_type == FieldType::FreeText)
                && f.non_null_ratio > 0.5
        })
        .map(|f| f.name.clone())
        .collect();

    let filter_fields: Vec<String> = fields
        .iter()
        .filter(|f| {
            f.unique_values.len() <= 50
                && !f.unique_values.is_empty()
                && f.field_type != FieldType::Boolean
                && f.field_type != FieldType::Nullable
                && f.field_type != FieldType::FreeText
        })
        .map(|f| f.name.clone())
        .collect();

    let bool_fields: Vec<String> = fields
        .iter()
        .filter(|f| f.field_type == FieldType::Boolean)
        .map(|f| f.name.clone())
        .collect();

    InferredSchema {
        fields,
        search_fields,
        filter_fields,
        bool_fields,
    }
}

#[derive(Default)]
struct FieldStats {
    total_seen: usize,
    null_count: usize,
    text_count: usize,
    number_count: usize,
    bool_count: usize,
    value_freq: HashMap<String, usize>,
}

// ════════════════════════════════════════════════════════════════════════════
// v0.5.0 — New input parsers
// ════════════════════════════════════════════════════════════════════════════

/// Parse NDJSON / JSONL bytes into a Vec<JsonRecord>.
///
/// - Each non-blank line is an independent JSON value.
/// - If the first non-blank line starts with `[` → delegate to serde_json.
/// - A malformed line returns `Err("Línea N: JSON inválido: …")`.
pub fn parse_ndjson(data: &[u8]) -> Result<Vec<JsonRecord>, String> {
    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 inválido: {e}"))?;

    // Detect JSON-array input and delegate.
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if first.trim_start().starts_with('[') {
        return serde_json::from_slice(data).map_err(|e| format!("Error parseando JSON: {e}"));
    }

    let mut records: Vec<JsonRecord> = Vec::new();
    for (idx, raw_line) in text.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let lineno = idx + 1;
        let value: serde_json::Value = serde_json::from_str(raw_line)
            .map_err(|e| format!("Línea {lineno}: JSON inválido: {e}"))?;
        records.push(value);
    }
    Ok(records)
}

/// Parse a SQLite database file and return one `(table_name, records)` entry
/// per user table, opened read-only.
///
/// - Skips internal `sqlite_*` tables.
/// - BLOB columns are hex-encoded with a `"0x"` prefix.
/// - Real values that are NaN/Infinity fall back to their string representation.
pub fn parse_sqlite(path: &std::path::Path) -> Result<Vec<(String, Vec<JsonRecord>)>, String> {
    use rusqlite::{types::Value as RV, Connection, OpenFlags};

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("No se pudo abrir el archivo SQLite: {e}"))?;

    // Enumerate user tables.
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master \
             WHERE type='table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .map_err(|e| format!("Error consultando sqlite_master: {e}"))?;

    let table_names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Error iterando tablas: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut result: Vec<(String, Vec<JsonRecord>)> = Vec::new();

    for table_name in table_names {
        let safe_name = table_name.replace('"', "\"\"");
        let sql = format!("SELECT * FROM \"{safe_name}\"");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("Error preparando SELECT en {table_name}: {e}"))?;

        let col_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let rows: Vec<Vec<RV>> = stmt
            .query_map([], |row| {
                (0..col_names.len())
                    .map(|i| row.get::<_, RV>(i))
                    .collect::<Result<Vec<RV>, _>>()
            })
            .map_err(|e| format!("Error consultando {table_name}: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        let records: Vec<JsonRecord> = rows
            .into_iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();
                for (col, val) in col_names.iter().zip(row.into_iter()) {
                    let json_val = match val {
                        RV::Null => serde_json::Value::Null,
                        RV::Integer(i) => serde_json::Value::Number(i.into()),
                        RV::Real(f) => serde_json::Number::from_f64(f)
                            .map(serde_json::Value::Number)
                            .unwrap_or_else(|| serde_json::Value::String(f.to_string())),
                        RV::Text(s) => serde_json::Value::String(s),
                        RV::Blob(b) => serde_json::Value::String(format!("0x{}", hex::encode(b))),
                    };
                    obj.insert(col.clone(), json_val);
                }
                serde_json::Value::Object(obj)
            })
            .collect();

        result.push((table_name, records));
    }

    Ok(result)
}

/// Parse XML bytes into a `Vec<JsonRecord>`.
///
/// - If root children all share the **same** tag name → each child is one record.
/// - Otherwise → flatten entire tree into 1 record with dot-notation keys.
pub fn parse_xml(data: &[u8]) -> Result<Vec<JsonRecord>, String> {
    use quick_xml::{events::Event, Reader};

    let text = std::str::from_utf8(data).map_err(|e| format!("UTF-8 inválido: {e}"))?;

    // First pass: collect root-level child tag names.
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut depth = 0usize;
    let mut root_child_tags: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth == 2 {
                    root_child_tags
                        .push(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                }
            }
            Ok(Event::End(_)) => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 1 {
                    root_child_tags
                        .push(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parseando XML: {e}")),
            _ => {}
        }
    }

    let all_same_tag =
        !root_child_tags.is_empty() && root_child_tags.iter().all(|t| t == &root_child_tags[0]);

    if all_same_tag {
        parse_xml_list(text)
    } else {
        parse_xml_flat(text).map(|rec| vec![rec])
    }
}

fn parse_xml_list(text: &str) -> Result<Vec<JsonRecord>, String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut records: Vec<JsonRecord> = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<serde_json::Map<String, serde_json::Value>> = None;
    let mut current_field: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth == 2 {
                    let mut obj = serde_json::Map::new();
                    for attr in e.attributes().flatten() {
                        let key =
                            String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        obj.insert(key, serde_json::Value::String(val));
                    }
                    current = Some(obj);
                } else if depth == 3 {
                    current_field =
                        Some(String::from_utf8_lossy(e.local_name().as_ref()).to_string());
                }
            }
            Ok(Event::Text(e)) => {
                if depth == 3 {
                    if let (Some(obj), Some(field)) = (current.as_mut(), current_field.as_ref()) {
                        let val = e.unescape().unwrap_or_default().to_string();
                        obj.insert(field.clone(), serde_json::Value::String(val));
                    }
                }
            }
            Ok(Event::End(_)) => {
                if depth == 3 {
                    current_field = None;
                } else if depth == 2 {
                    if let Some(obj) = current.take() {
                        records.push(serde_json::Value::Object(obj));
                    }
                }
                if depth > 0 {
                    depth -= 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 2 {
                    let field = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                    if let Some(obj) = current.as_mut() {
                        obj.insert(field, serde_json::Value::Null);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parseando XML: {e}")),
            _ => {}
        }
    }
    Ok(records)
}

fn parse_xml_flat(text: &str) -> Result<JsonRecord, String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut obj = serde_json::Map::new();
    let mut path_stack: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    let val = String::from_utf8_lossy(&attr.value).to_string();
                    let dot_key = if path_stack.is_empty() {
                        format!("{tag}.{key}")
                    } else {
                        format!("{}.{tag}.{key}", path_stack.join("."))
                    };
                    obj.insert(dot_key, serde_json::Value::String(val));
                }
                path_stack.push(tag);
            }
            Ok(Event::Text(e)) => {
                if !path_stack.is_empty() {
                    let val = e.unescape().unwrap_or_default().to_string();
                    if !val.is_empty() {
                        obj.insert(path_stack.join("."), serde_json::Value::String(val));
                    }
                }
            }
            Ok(Event::End(_)) => {
                path_stack.pop();
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                for attr in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                    let val = String::from_utf8_lossy(&attr.value).to_string();
                    let dot_key = if path_stack.is_empty() {
                        format!("{tag}.{key}")
                    } else {
                        format!("{}.{tag}.{key}", path_stack.join("."))
                    };
                    obj.insert(dot_key, serde_json::Value::String(val));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error parseando XML: {e}")),
            _ => {}
        }
    }
    Ok(serde_json::Value::Object(obj))
}

// ════════════════════════════════════════════════════════════════════════════
// v0.5.0 — Export functions
// ════════════════════════════════════════════════════════════════════════════

/// Export as pretty-printed JSON array (only visible columns).
pub fn export_to_json(
    records: &[&JsonRecord],
    visible_cols: &[String],
    path: &std::path::Path,
) -> Result<(), String> {
    let array: Vec<serde_json::Value> = records
        .iter()
        .map(|rec| {
            let mut obj = serde_json::Map::new();
            if let Some(src) = rec.as_object() {
                for col in visible_cols {
                    let val = src.get(col).cloned().unwrap_or(serde_json::Value::Null);
                    obj.insert(col.clone(), val);
                }
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    let json_str = serde_json::to_string_pretty(&array)
        .map_err(|e| format!("Error serializando JSON: {e}"))?;
    std::fs::write(path, json_str).map_err(|e| format!("Error escribiendo JSON: {e}"))
}

/// Export as xlsx workbook: row 1 = bold headers, rows 2..n = data.
pub fn export_to_xlsx(
    records: &[&JsonRecord],
    visible_cols: &[String],
    path: &std::path::Path,
) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    let bold = Format::new().set_bold();

    for (col_idx, col_name) in visible_cols.iter().enumerate() {
        sheet
            .write_string_with_format(0, col_idx as u16, col_name, &bold)
            .map_err(|e| format!("Error escribiendo cabecera xlsx: {e}"))?;
    }

    for (row_idx, rec) in records.iter().enumerate() {
        let obj = rec.as_object();
        for (col_idx, col_name) in visible_cols.iter().enumerate() {
            let val = obj
                .and_then(|o| o.get(col_name))
                .map(|v| crate::models::val_to_str(v))
                .unwrap_or_default();
            sheet
                .write_string(row_idx as u32 + 1, col_idx as u16, &val)
                .map_err(|e| format!("Error escribiendo dato xlsx: {e}"))?;
        }
    }

    workbook
        .save(path)
        .map_err(|e| format!("Error guardando xlsx: {e}"))
}

/// Export as GitHub Flavored Markdown table.
pub fn export_to_markdown(
    records: &[&JsonRecord],
    visible_cols: &[String],
    path: &std::path::Path,
) -> Result<(), String> {
    let mut out = String::new();

    // Header
    out.push('|');
    for col in visible_cols {
        out.push(' ');
        out.push_str(&col.replace('|', "\\|"));
        out.push_str(" |");
    }
    out.push('\n');

    // Separator
    out.push('|');
    for _ in visible_cols {
        out.push_str("---|");
    }
    out.push('\n');

    // Data
    for rec in records {
        let obj = rec.as_object();
        out.push('|');
        for col in visible_cols {
            let val = obj
                .and_then(|o| o.get(col))
                .map(|v| crate::models::val_to_str(v))
                .unwrap_or_default()
                .replace('|', "\\|");
            out.push(' ');
            out.push_str(&val);
            out.push_str(" |");
        }
        out.push('\n');
    }

    std::fs::write(path, out).map_err(|e| format!("Error escribiendo Markdown: {e}"))
}

/// Export as PDF table (A4 landscape) with mandatory column-overflow strategy:
/// 1. Compute max char width per column.
/// 2. Scale proportionally if total exceeds page width.
/// 3. Truncate individual cell values with "…".
/// 4. Add footer note when columns were truncated.
pub fn export_to_pdf(
    records: &[&JsonRecord],
    visible_cols: &[String],
    path: &std::path::Path,
    dataset_name: &str,
) -> Result<(), String> {
    use printpdf::{Line, Mm, PdfDocument, Point};

    // A4 landscape
    let page_w = 297.0_f32;
    let page_h = 210.0_f32;
    let margin = 10.0_f32;
    let safe_w = (page_w - 2.0 * margin) as f64;

    // Empirical: ~1.65 mm per character at 7pt Helvetica
    let char_w = 1.65_f64;
    let row_h = 5.5_f32;
    let hdr_h = 7.5_f32;
    let title_h = 9.0_f32;

    // Max chars per column before scaling
    let max_chars = 40usize;

    // ── Column widths in characters ──────────────────────────────────────────
    let mut col_chars: Vec<usize> = visible_cols
        .iter()
        .map(|col| {
            let hlen = col.chars().count();
            let dmax = records
                .iter()
                .filter_map(|r| r.as_object())
                .filter_map(|o| o.get(col))
                .map(|v| crate::models::val_to_str(v).chars().count())
                .max()
                .unwrap_or(0);
            hlen.max(dmax).min(max_chars)
        })
        .collect();

    // ── Scale down if too wide ───────────────────────────────────────────────
    let total_mm: f64 = col_chars.iter().map(|&c| c as f64 * char_w + 1.0).sum();
    let truncated = total_mm > safe_w;
    if truncated {
        let scale = safe_w / total_mm;
        for c in &mut col_chars {
            *c = ((*c as f64) * scale).floor() as usize;
            *c = (*c).max(3);
        }
    }
    let col_mm: Vec<f32> = col_chars
        .iter()
        .map(|&c| (c as f64 * char_w + 1.0) as f32)
        .collect();

    // ── PDF setup ────────────────────────────────────────────────────────────
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let title = format!("{dataset_name} — {now}");

    let (doc, page1, layer1) = PdfDocument::new(&title, Mm(page_w), Mm(page_h), "Layer 1");
    let layer = doc.get_page(page1).get_layer(layer1);

    let font = doc
        .add_builtin_font(printpdf::BuiltinFont::Helvetica)
        .map_err(|e| format!("Error cargando fuente PDF: {e}"))?;
    let font_bold = doc
        .add_builtin_font(printpdf::BuiltinFont::HelveticaBold)
        .map_err(|e| format!("Error cargando fuente PDF: {e}"))?;

    let truncate_cell = |text: &str, max: usize| -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= max || max == 0 {
            text.to_string()
        } else if max <= 1 {
            "…".to_string()
        } else {
            format!("{}…", chars[..max - 1].iter().collect::<String>())
        }
    };

    // ── Draw helper ──────────────────────────────────────────────────────────
    let draw_hline = |layer: &printpdf::PdfLayerReference, y: f32| {
        let line = Line {
            points: vec![
                (Point::new(Mm(margin), Mm(y)), false),
                (Point::new(Mm(page_w - margin), Mm(y)), false),
            ],
            is_closed: false,
        };
        layer.add_line(line);
    };

    let rows_per_page = ((page_h - margin * 2.0 - title_h - hdr_h - 10.0) / row_h).floor() as usize;

    let mut y = page_h - margin;

    // Title
    layer.use_text(&title, 10.0, Mm(margin), Mm(y - title_h + 1.5), &font_bold);
    y -= title_h + 1.0;

    // Header
    let mut x = margin;
    for (i, col) in visible_cols.iter().enumerate() {
        let text = truncate_cell(col, col_chars[i]);
        layer.use_text(&text, 7.0, Mm(x), Mm(y - hdr_h + 2.0), &font_bold);
        x += col_mm[i];
    }
    draw_hline(&layer, y - hdr_h + 0.5);
    y -= hdr_h;

    let mut current_layer = layer;
    let mut row_on_page = 0usize;

    for rec in records {
        if row_on_page >= rows_per_page {
            // New page
            let (new_page, new_layer_idx) = doc.add_page(Mm(page_w), Mm(page_h), "Layer 1");
            let new_layer = doc.get_page(new_page).get_layer(new_layer_idx);

            y = page_h - margin;
            x = margin;
            for (i, col) in visible_cols.iter().enumerate() {
                let text = truncate_cell(col, col_chars[i]);
                new_layer.use_text(&text, 7.0, Mm(x), Mm(y - hdr_h + 2.0), &font_bold);
                x += col_mm[i];
            }
            draw_hline(&new_layer, y - hdr_h + 0.5);
            y -= hdr_h;

            current_layer = new_layer;
            row_on_page = 0;
        }

        let obj = rec.as_object();
        x = margin;
        for (i, col) in visible_cols.iter().enumerate() {
            let raw = obj
                .and_then(|o| o.get(col))
                .map(|v| crate::models::val_to_str(v))
                .unwrap_or_default();
            let text = truncate_cell(&raw, col_chars[i]);
            current_layer.use_text(&text, 7.0, Mm(x), Mm(y - row_h + 2.0), &font);
            x += col_mm[i];
        }
        y -= row_h;
        row_on_page += 1;
    }

    // Footer note
    if truncated {
        current_layer.use_text(
            "Tabla truncada — exporta como Excel para ver todas las columnas",
            6.0,
            Mm(margin),
            Mm(margin + 2.0),
            &font,
        );
    }

    let bytes = doc
        .save_to_bytes()
        .map_err(|e| format!("Error generando PDF: {e}"))?;
    std::fs::write(path, bytes).map_err(|e| format!("Error escribiendo PDF: {e}"))
}

// ════════════════════════════════════════════════════════════════════════════
// Unit tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── NDJSON ───────────────────────────────────────────────────────────────

    #[test]
    fn ndjson_round_trip() {
        let input = b"{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n";
        let records = parse_ndjson(input).expect("parse ok");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["a"], serde_json::json!(1));
        assert_eq!(records[2]["a"], serde_json::json!(3));
    }

    #[test]
    fn ndjson_skips_empty_lines() {
        let input = b"{\"x\":1}\n\n   \n{\"x\":2}\n";
        let records = parse_ndjson(input).expect("parse ok");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn ndjson_error_on_bad_line() {
        let input = b"{\"ok\":1}\nNOT JSON\n{\"ok\":2}\n";
        let err = parse_ndjson(input).expect_err("should fail");
        assert!(
            err.contains("Línea 2"),
            "error should mention line 2, got: {err}"
        );
    }

    // ── XML ──────────────────────────────────────────────────────────────────

    #[test]
    fn xml_list_structure() {
        let xml = br#"<root><item><name>Alice</name><age>30</age></item><item><name>Bob</name><age>25</age></item></root>"#;
        let records = parse_xml(xml).expect("parse ok");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["name"], serde_json::json!("Alice"));
        assert_eq!(records[1]["age"], serde_json::json!("25"));
    }

    #[test]
    fn xml_single_record() {
        let xml =
            br#"<config><database><host>localhost</host><port>5432</port></database></config>"#;
        let records = parse_xml(xml).expect("parse ok");
        assert_eq!(records.len(), 1);
        let rec = &records[0];
        // Accept any dot-notation key that contains "host"
        let has_host = rec
            .as_object()
            .map(|o| o.keys().any(|k| k.contains("host")))
            .unwrap_or(false);
        assert!(
            has_host,
            "expected dot-notation key with 'host', got: {rec:?}"
        );
    }

    // ── JSON export ──────────────────────────────────────────────────────────

    #[test]
    fn export_json_round_trip() {
        let r1 = serde_json::json!({"name": "Alice", "age": 30});
        let r2 = serde_json::json!({"name": "Bob",   "age": 25});
        let refs: Vec<&JsonRecord> = vec![&r1, &r2];
        let cols = vec!["name".to_string(), "age".to_string()];

        let tmp = tempfile::NamedTempFile::new().unwrap();
        export_to_json(&refs, &cols, tmp.path()).expect("export ok");

        let bytes = std::fs::read(tmp.path()).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["name"], serde_json::json!("Alice"));
        assert_eq!(parsed[1]["age"], serde_json::json!(25));
    }

    // ── Markdown export ──────────────────────────────────────────────────────

    #[test]
    fn export_markdown_format() {
        let r1 = serde_json::json!({"col1": "v1", "col2": "v2"});
        let refs: Vec<&JsonRecord> = vec![&r1];
        let cols = vec!["col1".to_string(), "col2".to_string()];

        let tmp = tempfile::NamedTempFile::new().unwrap();
        export_to_markdown(&refs, &cols, tmp.path()).expect("export ok");

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains('|'), "missing pipes: {content}");
        assert!(content.contains("---|"), "missing separator row: {content}");
        assert!(content.contains("v1"), "missing data value: {content}");
    }
}
