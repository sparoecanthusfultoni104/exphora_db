use crate::commands::file_ops::compute_unique_values_impl;
use crate::filters::apply_filters as run_filters;
use crate::models::{
    compute_stats, ColumnStats, Connector, EasyFilter, FilterMode, FilterOp, FilterRule, JsonRecord,
};
use crate::parser::infer_schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Serializable filter types (mirrors Rust models for JSON round-trip) ──────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FilterRuleDto {
    pub op: String,
    pub value: String,
    pub connector: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EasyFilterDto {
    pub selected: Vec<String>,
    pub all_selected: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DynamicFiltersDto {
    pub text_search: String,
    pub filters: HashMap<String, Vec<FilterRuleDto>>,
    pub easy_filters: HashMap<String, EasyFilterDto>,
    pub filter_mode: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterResult {
    pub filtered_indices: Vec<usize>,
    pub total_matching: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UniqueValuesResult {
    pub col: String,
    pub values: Vec<(String, usize)>,
    pub truncated: bool,
}

// ── DTO → Domain conversions ──────────────────────────────────────────────────

fn op_from_str(s: &str) -> FilterOp {
    match s {
        "NotContains" => FilterOp::NotContains,
        "Equals" => FilterOp::Equals,
        "NotEquals" => FilterOp::NotEquals,
        "GreaterThan" => FilterOp::GreaterThan,
        "LessThan" => FilterOp::LessThan,
        "IsNull" => FilterOp::IsNull,
        "IsNotNull" => FilterOp::IsNotNull,
        "BoolTrue" => FilterOp::BoolTrue,
        _ => FilterOp::Contains,
    }
}

fn connector_from_str(s: &str) -> Connector {
    if s == "Or" {
        Connector::Or
    } else {
        Connector::And
    }
}

fn dto_to_filters(dto: DynamicFiltersDto) -> crate::filters::DynamicFilters {
    let filters: HashMap<String, Vec<FilterRule>> = dto
        .filters
        .into_iter()
        .map(|(col, rules)| {
            let converted = rules
                .into_iter()
                .map(|r| FilterRule {
                    op: op_from_str(&r.op),
                    value: r.value,
                    connector: connector_from_str(&r.connector),
                })
                .collect();
            (col, converted)
        })
        .collect();

    let easy_filters: HashMap<String, EasyFilter> = dto
        .easy_filters
        .into_iter()
        .map(|(col, ef)| {
            (
                col,
                EasyFilter {
                    selected: ef.selected.into_iter().collect(),
                    all_selected: ef.all_selected,
                },
            )
        })
        .collect();

    let filter_mode: HashMap<String, FilterMode> = dto
        .filter_mode
        .into_iter()
        .map(|(col, mode)| {
            let m = if mode == "Advanced" {
                FilterMode::Advanced
            } else {
                FilterMode::Easy
            };
            (col, m)
        })
        .collect();

    crate::filters::DynamicFilters {
        text_search: dto.text_search,
        filters,
        easy_filters,
        filter_mode,
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Apply all active filters and return the matched record indices.
#[tauri::command]
pub fn apply_filters(
    records: Vec<JsonRecord>,
    filters_dto: DynamicFiltersDto,
) -> Result<FilterResult, String> {
    let schema = infer_schema(&records);
    let dynamic = dto_to_filters(filters_dto);
    let filtered_indices: Vec<usize> = run_filters(&dynamic, &schema, &records);
    let total_matching = filtered_indices.len();
    Ok(FilterResult {
        filtered_indices,
        total_matching,
    })
}

/// Compute (value, count) pairs for `col` within `filtered_indices`.
/// Returns at most 200 values, sorted by descending frequency.
#[tauri::command]
pub fn get_unique_values(
    col: String,
    records: Vec<JsonRecord>,
    filtered_indices: Vec<usize>,
) -> Result<UniqueValuesResult, String> {
    const MAX_UNIQUE: usize = 200;
    let all = compute_unique_values_impl(&col, &records, &filtered_indices, MAX_UNIQUE + 1);
    let truncated = all.len() > MAX_UNIQUE;
    let values = all.into_iter().take(MAX_UNIQUE).collect();
    Ok(UniqueValuesResult {
        col,
        values,
        truncated,
    })
}

/// Compute column statistics (numeric stats + top values).
#[tauri::command]
pub fn get_column_stats(
    col: String,
    records: Vec<JsonRecord>,
    filtered_indices: Vec<usize>,
) -> Result<ColumnStats, String> {
    Ok(compute_stats(&records, &filtered_indices, &col))
}
