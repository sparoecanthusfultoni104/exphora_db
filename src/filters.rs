use crate::models::{
    apply_easy_filter, eval_rules, val_to_str, EasyFilter, FilterMode, FilterRule, InferredSchema,
    JsonRecord,
};
use std::collections::HashMap;

/// Dynamic filter state for any dataset.
#[derive(Clone, Default)]
pub struct DynamicFilters {
    pub text_search: String,
    /// Per-column Advanced-mode rule lists. Empty vec = no filter for that column.
    pub filters: HashMap<String, Vec<FilterRule>>,
    /// Per-column Easy-mode filters.
    pub easy_filters: HashMap<String, EasyFilter>,
    /// Which mode is active per column.
    pub filter_mode: HashMap<String, FilterMode>,
}

impl DynamicFilters {
    pub fn reset(&mut self) {
        self.text_search.clear();
        self.filters.clear();
        self.easy_filters.clear();
        self.filter_mode.clear();
    }
}

// ── Advanced search query parsing ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SearchTerm {
    FreeText(String),
    FieldValue(String, String), // (field_name, value)
}

/// Parse a search query supporting:
///   campo: "valor"         → FieldValue
///   campo: valor           → FieldValue
///   campo1: "v1" campo2: "v2" → multiple FieldValue
///   texto libre            → FreeText
pub fn parse_search_query(input: &str) -> Vec<SearchTerm> {
    let mut terms = Vec::new();
    let mut remaining = input.trim();

    while !remaining.is_empty() {
        // Try to match field:value pattern
        if let Some(colon_pos) = remaining.find(':') {
            let before_colon = &remaining[..colon_pos];
            // The field name is the last word before the colon
            let field_candidate = before_colon.trim();

            // Check if field_candidate is a single word (no spaces except leading free text)
            if let Some(space_pos) = field_candidate.rfind(char::is_whitespace) {
                // There's free text before the field name
                let free_text = field_candidate[..space_pos].trim();
                if !free_text.is_empty() {
                    terms.push(SearchTerm::FreeText(free_text.to_string()));
                }
                let field_name = field_candidate[space_pos..].trim();
                let after_colon = remaining[colon_pos + 1..].trim_start();

                let (value, rest) = extract_value(after_colon);
                if !value.is_empty() {
                    terms.push(SearchTerm::FieldValue(field_name.to_string(), value));
                }
                remaining = rest.trim();
            } else if !field_candidate.is_empty() {
                // The entire before-colon part is the field name
                let field_name = field_candidate;
                let after_colon = remaining[colon_pos + 1..].trim_start();

                let (value, rest) = extract_value(after_colon);
                if !value.is_empty() {
                    terms.push(SearchTerm::FieldValue(field_name.to_string(), value));
                }
                remaining = rest.trim();
            } else {
                // Empty field name, treat as free text
                terms.push(SearchTerm::FreeText(remaining.to_string()));
                break;
            }
        } else {
            // No colon found → everything is free text
            terms.push(SearchTerm::FreeText(remaining.to_string()));
            break;
        }
    }

    terms
}

/// Extract a value after the colon. Supports quoted and unquoted values.
fn extract_value(s: &str) -> (String, &str) {
    if s.starts_with('"') {
        // Quoted value: find closing quote
        if let Some(end_quote) = s[1..].find('"') {
            let value = s[1..1 + end_quote].to_string();
            let rest = &s[1 + end_quote + 1..];
            (value, rest)
        } else {
            // No closing quote — take rest as value
            (s[1..].to_string(), "")
        }
    } else {
        // Unquoted value: take until next space
        if let Some(space_pos) = s.find(char::is_whitespace) {
            let value = s[..space_pos].to_string();
            let rest = &s[space_pos..];
            (value, rest)
        } else {
            (s.to_string(), "")
        }
    }
}

// ── Filtering ────────────────────────────────────────────────────────────────

/// Apply all active filters and return indices of matching records.
pub fn apply_filters(
    filters: &DynamicFilters,
    schema: &InferredSchema,
    records: &[JsonRecord],
) -> Vec<usize> {
    let search_terms = parse_search_query(&filters.text_search);

    records
        .iter()
        .enumerate()
        .filter(|(_i, record)| {
            let obj = match record.as_object() {
                Some(o) => o,
                None => return false,
            };

            // Advanced text search
            if !search_terms.is_empty() {
                let matches = search_terms.iter().all(|term| match term {
                    SearchTerm::FreeText(text) => {
                        let text_lower = text.to_lowercase();
                        schema.search_fields.iter().any(|field_name| {
                            obj.get(field_name)
                                .map(|v| val_to_str(v).to_lowercase().contains(&text_lower))
                                .unwrap_or(false)
                        })
                    }
                    SearchTerm::FieldValue(field, value) => {
                        let value_lower = value.to_lowercase();
                        obj.get(field)
                            .map(|v| val_to_str(v).to_lowercase().contains(&value_lower))
                            .unwrap_or(false)
                    }
                });
                if !matches {
                    return false;
                }
            }

            // Per-column filters — dispatch by mode
            // Collect all column names that appear in either filter map
            let all_cols: std::collections::HashSet<&String> = filters
                .filters
                .keys()
                .chain(filters.easy_filters.keys())
                .collect();

            for col in all_cols {
                // Si la columna no aparece en filter_mode se asume FilterMode::Easy (default).
                let mode = filters
                    .filter_mode
                    .get(col)
                    .cloned()
                    .unwrap_or(FilterMode::Easy);

                let cell = obj.get(col).map(|v| val_to_str(v)).unwrap_or_default();

                match mode {
                    FilterMode::Easy => {
                        if let Some(ef) = filters.easy_filters.get(col) {
                            if !apply_easy_filter(ef, &cell) {
                                return false;
                            }
                        }
                        // If no EasyFilter entry exists, no filter for this col → pass
                    }
                    FilterMode::Advanced => {
                        if let Some(rules) = filters.filters.get(col) {
                            if rules.is_empty() {
                                continue;
                            }
                            if !eval_rules(rules, &cell) {
                                return false;
                            }
                        }
                    }
                }
            }

            true
        })
        .map(|(i, _)| i)
        .collect()
}

/// Sort filtered indices by a column value.
pub fn sort_indices(indices: &mut [usize], records: &[JsonRecord], column: &str, ascending: bool) {
    indices.sort_by(|&a, &b| {
        let va = records
            .get(a)
            .and_then(|r| r.as_object())
            .and_then(|o| o.get(column))
            .map(|v| val_to_str(v))
            .unwrap_or_default();
        let vb = records
            .get(b)
            .and_then(|r| r.as_object())
            .and_then(|o| o.get(column))
            .map(|v| val_to_str(v))
            .unwrap_or_default();

        let cmp = match (va.parse::<f64>(), vb.parse::<f64>()) {
            (Ok(na), Ok(nb)) => na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal),
            _ => va.to_lowercase().cmp(&vb.to_lowercase()),
        };

        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });
}
