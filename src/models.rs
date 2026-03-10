use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A generic JSON record — any JSON object.
pub type JsonRecord = serde_json::Value;

// ── Field type inference ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Number,
    Boolean,
    Nullable,
    FreeText, // text with >500 unique values — no dropdown
}

// FilterPanel has been removed — filter panel state is now managed entirely by the
// React frontend. The Tauri commands supply data (unique values, etc.) on demand.

#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub name: String,
    pub field_type: FieldType,
    pub unique_values: Vec<String>,
    pub non_null_ratio: f64,
    pub distinct_count: usize,
}

#[derive(Debug, Clone)]
pub struct InferredSchema {
    pub fields: Vec<FieldMeta>,
    pub search_fields: Vec<String>,
    pub filter_fields: Vec<String>,
    pub bool_fields: Vec<String>,
}

// ── Column statistics ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ColumnStats {
    pub total: usize,
    pub non_null: usize,
    pub unique: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub top_values: Vec<(String, usize)>, // (value_display, count)
    pub is_numeric: bool,
}

/// Compute statistics for `col` over the records at `indices`.
/// Calculates numeric stats only if every non-null value parses as f64.
pub fn compute_stats(records: &[JsonRecord], indices: &[usize], col: &str) -> ColumnStats {
    let total = indices.len();
    let mut freq: HashMap<String, usize> = HashMap::new();
    let mut non_null = 0usize;
    let mut numerics: Vec<f64> = Vec::new();
    let mut all_numeric = true;

    for &idx in indices {
        let raw = records
            .get(idx)
            .and_then(|r| r.as_object())
            .and_then(|o| o.get(col));

        let text = match raw {
            None | Some(serde_json::Value::Null) => {
                // count as null
                *freq.entry("null".to_string()).or_insert(0) += 1;
                continue;
            }
            Some(v) => val_to_str(v),
        };

        if text.is_empty() {
            *freq.entry("null".to_string()).or_insert(0) += 1;
            continue;
        }

        non_null += 1;
        *freq.entry(text.clone()).or_insert(0) += 1;

        match text.parse::<f64>() {
            Ok(n) => numerics.push(n),
            Err(_) => all_numeric = false,
        }
    }

    let unique = freq.len();

    // Numeric stats
    let is_numeric = all_numeric && !numerics.is_empty();
    let (min, max, mean, median) = if is_numeric {
        let mn = numerics.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = numerics.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = numerics.iter().sum::<f64>() / numerics.len() as f64;
        numerics.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let med = {
            let n = numerics.len();
            if n % 2 == 1 {
                numerics[n / 2]
            } else {
                (numerics[n / 2 - 1] + numerics[n / 2]) / 2.0
            }
        };
        (Some(mn), Some(mx), Some(avg), Some(med))
    } else {
        (None, None, None, None)
    };

    // Top 5 by frequency descending
    let mut top: Vec<(String, usize)> = freq.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top.truncate(5);

    ColumnStats {
        total,
        non_null,
        unique,
        min,
        max,
        mean,
        median,
        top_values: top,
        is_numeric,
    }
}

// ── Dataset and tab state ────────────────────────────────────────────────────

pub struct LoadedDataset {
    pub name: String,
    pub path: String,
    pub records: Vec<JsonRecord>,
    pub schema: InferredSchema,
}

// ── Easy/Advanced filter mode ───────────────────────────────────────────────

/// Which mode is active for a given column's filter panel.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    Easy,
    Advanced,
}

/// Easy-mode filter: a set of selected unique values.
/// `all_selected = true` means no filter (show everything).
#[derive(Clone, Debug)]
pub struct EasyFilter {
    /// The values currently checked (included in results).
    pub selected: HashSet<String>,
    /// True when the user has not deselected anything — no filter active.
    pub all_selected: bool,
}

impl Default for EasyFilter {
    fn default() -> Self {
        EasyFilter {
            selected: HashSet::new(),
            all_selected: true,
        }
    }
}

impl EasyFilter {
    /// Build from a complete list of unique values (all checked by default).
    pub fn all_checked(values: impl IntoIterator<Item = String>) -> Self {
        EasyFilter {
            selected: values.into_iter().collect(),
            all_selected: true,
        }
    }
}

/// Returns true if `cell` passes the EasyFilter.
/// Empty cell is matched against the sentinel string "null".
pub fn apply_easy_filter(ef: &EasyFilter, cell: &str) -> bool {
    if ef.all_selected {
        return true;
    }
    let key = if cell.is_empty() { "null" } else { cell };
    ef.selected.contains(key)
}

// ── Multi-filter types ───────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Connector {
    #[default]
    And,
    Or,
}

impl Connector {
    pub fn label(&self) -> &'static str {
        match self {
            Connector::And => "AND",
            Connector::Or => "OR",
        }
    }
    pub fn toggle(&mut self) {
        *self = match self {
            Connector::And => Connector::Or,
            Connector::Or => Connector::And,
        };
    }
}

/// Filter operations supported in the rule editor.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum FilterOp {
    #[default]
    Contains,
    NotContains,
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    IsNull,
    IsNotNull,
    BoolTrue, // boolean field shortcut
}

impl FilterOp {
    pub fn label(&self) -> &'static str {
        match self {
            FilterOp::Contains => "contiene",
            FilterOp::NotContains => "no contiene",
            FilterOp::Equals => "igual a",
            FilterOp::NotEquals => "diferente de",
            FilterOp::GreaterThan => "mayor que",
            FilterOp::LessThan => "menor que",
            FilterOp::IsNull => "es nulo",
            FilterOp::IsNotNull => "no es nulo",
            FilterOp::BoolTrue => "es verdadero",
        }
    }

    pub fn all_text() -> &'static [FilterOp] {
        &[
            FilterOp::Contains,
            FilterOp::NotContains,
            FilterOp::Equals,
            FilterOp::NotEquals,
            FilterOp::GreaterThan,
            FilterOp::LessThan,
            FilterOp::IsNull,
            FilterOp::IsNotNull,
        ]
    }

    /// True if this op requires a text input value.
    pub fn needs_value(&self) -> bool {
        !matches!(
            self,
            FilterOp::IsNull | FilterOp::IsNotNull | FilterOp::BoolTrue
        )
    }

    /// Evaluate this op against a cell value string.
    pub fn matches(&self, cell: &str, input: &str) -> bool {
        match self {
            FilterOp::Contains => cell.to_lowercase().contains(&input.to_lowercase()),
            FilterOp::NotContains => !cell.to_lowercase().contains(&input.to_lowercase()),
            FilterOp::Equals => cell.eq_ignore_ascii_case(input),
            FilterOp::NotEquals => !cell.eq_ignore_ascii_case(input),
            FilterOp::GreaterThan => match (cell.parse::<f64>(), input.parse::<f64>()) {
                (Ok(a), Ok(b)) => a > b,
                _ => cell > input,
            },
            FilterOp::LessThan => match (cell.parse::<f64>(), input.parse::<f64>()) {
                (Ok(a), Ok(b)) => a < b,
                _ => cell < input,
            },
            FilterOp::IsNull => cell.is_empty(),
            FilterOp::IsNotNull => !cell.is_empty(),
            FilterOp::BoolTrue => {
                matches!(cell.to_lowercase().as_str(), "true" | "1" | "si" | "yes")
            }
        }
    }
}

/// A single filter condition for a column.
#[derive(Clone, Debug, Default)]
pub struct FilterRule {
    pub op: FilterOp,
    pub value: String,
    /// Connector to apply with the NEXT rule (ignored for the last rule).
    pub connector: Connector,
}

impl FilterRule {
    pub fn new(op: FilterOp) -> Self {
        FilterRule {
            op,
            value: String::new(),
            connector: Connector::And,
        }
    }
}

/// Evaluate a Vec<FilterRule> against a cell value.
/// Returns true if the record passes (empty Vec = always pass).
pub fn eval_rules(rules: &[FilterRule], cell: &str) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut result = rules[0].op.matches(cell, &rules[0].value);
    for i in 1..rules.len() {
        let next = rules[i].op.matches(cell, &rules[i].value);
        result = match rules[i - 1].connector {
            Connector::And => result && next,
            Connector::Or => result || next,
        };
    }
    result
}

// Keep FilterValue for backwards compat with DynamicFilters (boolean fields)
#[derive(Clone, Debug, PartialEq)]
pub enum FilterValue {
    Selected(String),
    BoolTrue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

pub struct TabState {
    /// Per-column multi-filter rules (Advanced mode). Empty Vec = no filter.
    pub filters: std::collections::HashMap<String, Vec<FilterRule>>,
    /// Per-column Easy-mode filters.
    pub easy_filters: HashMap<String, EasyFilter>,
    /// Which mode is active per column (default: Easy).
    pub filter_mode: HashMap<String, FilterMode>,
    pub text_search: String,
    pub filtered_indices: Vec<usize>,
    pub selected_row: Option<usize>,
    pub sort_column: Option<String>,
    pub sort_dir: SortDir,
    pub show_stats: bool,
    pub stats_field: String,
    pub show_columns_panel: bool,
    pub visible_columns: HashMap<String, bool>,

    // ── Feature: column statistics panel ─────────────────────────────────────
    /// Stats for the column currently being inspected (col_name, stats)
    pub active_stats: Option<(String, ColumnStats)>,

    // ── Feature: frozen columns ───────────────────────────────────────────────
    /// Names of pinned columns, rendered first (max 5)
    pub frozen_cols: Vec<String>,

    // ── Feature: calculated columns ──────────────────────────────────────────
    /// (column_name, expression) — definition
    pub calculated_cols: Vec<(String, String)>,
    /// Cached evaluated values: col_name → Vec<Option<String>> indexed by record index
    pub calc_col_cache: HashMap<String, Vec<Option<String>>>,
    /// True when cache is stale and must be recomputed
    pub calc_col_dirty: bool,
    /// Editor state while the "new column" window is open
    pub calc_col_editor: Option<(String, String)>,

    // ── Feature: per-column widths (scroll perf) ──────────────────────────────
    /// Sampled column width. Populated once when dataset loads, or on demand.
    pub col_widths: HashMap<String, f32>,
}

impl Default for TabState {
    fn default() -> Self {
        Self {
            filters: std::collections::HashMap::new(),
            easy_filters: HashMap::new(),
            filter_mode: HashMap::new(),
            text_search: String::new(),
            filtered_indices: Vec::new(),
            selected_row: None,
            sort_column: None,
            sort_dir: SortDir::Asc,
            show_stats: false,
            stats_field: String::new(),
            show_columns_panel: false,
            visible_columns: HashMap::new(),
            active_stats: None,
            frozen_cols: Vec::new(),
            calculated_cols: Vec::new(),
            calc_col_cache: HashMap::new(),
            calc_col_dirty: false,
            calc_col_editor: None,
            col_widths: HashMap::new(),
        }
    }
}

impl TabState {
    pub fn init_visible_columns(&mut self, schema: &InferredSchema) {
        self.visible_columns.clear();
        for field in &schema.fields {
            self.visible_columns.insert(field.name.clone(), true);
        }
    }

    /// Returns the names of visible columns in schema order, excluding hidden ones.
    pub fn get_visible_columns(&self, schema: &InferredSchema) -> Vec<String> {
        schema
            .fields
            .iter()
            .filter(|f| *self.visible_columns.get(&f.name).unwrap_or(&true))
            .map(|f| f.name.clone())
            .collect()
    }

    /// Sample column widths from the first `sample` records (monospace approx: 8px/char).
    pub fn sample_col_widths(
        &mut self,
        schema: &InferredSchema,
        records: &[JsonRecord],
        sample: usize,
    ) {
        const MIN_W: f32 = 80.0;
        const MAX_W: f32 = 260.0;
        const CHAR_PX: f32 = 8.0;
        const HDR_PAD: f32 = 16.0;

        for field in &schema.fields {
            let hdr_w = field.name.len() as f32 * CHAR_PX + HDR_PAD;
            let max_content = records
                .iter()
                .take(sample)
                .filter_map(|r| r.as_object())
                .filter_map(|o| o.get(&field.name))
                .map(|v| val_to_str(v).len())
                .max()
                .unwrap_or(0) as f32
                * CHAR_PX
                + HDR_PAD;
            let w = hdr_w.max(max_content).clamp(MIN_W, MAX_W);
            self.col_widths.entry(field.name.clone()).or_insert(w);
        }
    }
}

// ── Config persistence ───────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct TabConfig {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub tabs: Vec<TabConfig>,
    pub active_tab: usize,
}

// ── JSON value helpers ───────────────────────────────────────────────────────

pub fn val_to_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

pub fn val_to_bool(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        serde_json::Value::String(s) => {
            matches!(s.to_lowercase().as_str(), "true" | "1" | "si" | "yes")
        }
        _ => false,
    }
}

pub fn record_all_fields(record: &JsonRecord) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(obj) = record.as_object() {
        for (k, v) in obj {
            let s = val_to_str(v);
            if !s.is_empty() {
                fields.push((k.clone(), s));
            }
        }
    }
    fields
}

pub fn record_title(record: &JsonRecord) -> String {
    if let Some(obj) = record.as_object() {
        let name_parts: Vec<&str> = ["nombre1", "nombre2", "apellido1", "apellido2"]
            .iter()
            .filter_map(|k| obj.get(*k))
            .filter_map(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        if !name_parts.is_empty() {
            return name_parts.join(" ");
        }
        for (_k, v) in obj {
            if let Some(s) = v.as_str() {
                if !s.is_empty() && s.len() < 80 {
                    return s.to_string();
                }
            }
        }
    }
    "Registro".to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_records(vals: &[&str]) -> Vec<JsonRecord> {
        vals.iter().map(|v| serde_json::json!({"col": v})).collect()
    }

    #[test]
    fn stats_numeric_col() {
        let recs = make_records(&["1", "2", "3", "4", "5"]);
        let indices: Vec<usize> = (0..recs.len()).collect();
        let s = compute_stats(&recs, &indices, "col");
        assert!(s.is_numeric);
        assert_eq!(s.total, 5);
        assert_eq!(s.non_null, 5);
        assert!((s.mean.unwrap() - 3.0).abs() < 1e-9);
        assert!((s.median.unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn stats_text_col() {
        let recs = make_records(&["hola", "mundo", "foo"]);
        let indices: Vec<usize> = (0..recs.len()).collect();
        let s = compute_stats(&recs, &indices, "col");
        assert!(!s.is_numeric);
        assert_eq!(s.total, 3);
        assert_eq!(s.non_null, 3);
    }

    #[test]
    fn stats_null_handling() {
        let recs: Vec<JsonRecord> = vec![
            serde_json::json!({"col": "1"}),
            serde_json::json!({"col": null}),
            serde_json::json!({"col": "3"}),
        ];
        let indices: Vec<usize> = (0..recs.len()).collect();
        let s = compute_stats(&recs, &indices, "col");
        assert_eq!(s.total, 3);
        assert!(s.non_null < s.total);
    }

    // ── Multi-filter tests ───────────────────────────────────────────────────

    #[test]
    fn filter_multi_and() {
        // precio > 100 AND precio < 500 — only records in that range pass
        let rules = vec![
            FilterRule {
                op: FilterOp::GreaterThan,
                value: "100".to_string(),
                connector: Connector::And,
            },
            FilterRule {
                op: FilterOp::LessThan,
                value: "500".to_string(),
                connector: Connector::And,
            },
        ];

        assert!(eval_rules(&rules, "200"), "200 should pass");
        assert!(eval_rules(&rules, "499"), "499 should pass");
        assert!(!eval_rules(&rules, "50"), "50 should fail (too low)");
        assert!(
            !eval_rules(&rules, "100"),
            "100 should fail (not strictly greater)"
        );
        assert!(
            !eval_rules(&rules, "500"),
            "500 should fail (not strictly less)"
        );
        assert!(!eval_rules(&rules, "600"), "600 should fail (too high)");
    }

    #[test]
    fn filter_multi_or() {
        // status = "active" OR status = "pending"
        let rules = vec![
            FilterRule {
                op: FilterOp::Equals,
                value: "active".to_string(),
                connector: Connector::Or,
            },
            FilterRule {
                op: FilterOp::Equals,
                value: "pending".to_string(),
                connector: Connector::And,
            },
        ];

        assert!(eval_rules(&rules, "active"), "active should pass");
        assert!(eval_rules(&rules, "pending"), "pending should pass");
        assert!(!eval_rules(&rules, "inactive"), "inactive should fail");
        assert!(!eval_rules(&rules, "done"), "done should fail");
    }

    // ── Easy-filter tests ────────────────────────────────────────────────────

    #[test]
    fn easy_filter_excludes_value() {
        let mut ef = EasyFilter {
            selected: ["active".to_string(), "pending".to_string()]
                .into_iter()
                .collect(),
            all_selected: false,
        };
        assert!(apply_easy_filter(&ef, "active"), "active should pass");
        assert!(apply_easy_filter(&ef, "pending"), "pending should pass");
        assert!(
            !apply_easy_filter(&ef, "closed"),
            "closed should be excluded"
        );
        assert!(
            !apply_easy_filter(&ef, ""),
            "empty (null) should be excluded"
        );
        // Re-add null
        ef.selected.insert("null".to_string());
        assert!(
            apply_easy_filter(&ef, ""),
            "null should pass after re-adding"
        );
    }

    #[test]
    fn easy_filter_all_selected_no_filter() {
        let ef = EasyFilter {
            selected: HashSet::new(),
            all_selected: true,
        };
        assert!(apply_easy_filter(&ef, "active"));
        assert!(apply_easy_filter(&ef, ""));
        assert!(apply_easy_filter(&ef, "anything"));
    }
}
