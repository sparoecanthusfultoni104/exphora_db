use crate::expr::eval_expr;
use crate::models::{val_to_str, JsonRecord};
use std::collections::HashMap;

/// Evaluate a calculated column expression against every record.
/// Returns `None` for rows where the expression evaluates to Null.
#[tauri::command]
pub fn eval_calc_column(
    expr_str: String,
    records: Vec<JsonRecord>,
) -> Result<Vec<Option<String>>, String> {
    let results = records
        .iter()
        .map(|record| {
            // Build a flat String → String map from the record's fields
            let row_map: HashMap<String, String> = record
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), val_to_str(v)))
                        .collect()
                })
                .unwrap_or_default();

            eval_expr(&expr_str, &row_map).to_display()
        })
        .collect();

    Ok(results)
}
