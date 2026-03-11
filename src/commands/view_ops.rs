use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewState {
    pub dataset_path: String,
    pub filters: Value,
    pub text_search: String,
    pub visible_columns: Value,
    pub frozen_cols: Vec<String>,
    pub calc_cols: Value,
    pub sort_col: Option<String>,
    pub sort_asc: bool,
    pub show_frequency_chart: bool,
    pub frequency_chart_col: Option<String>,
    #[serde(default)]
    pub charts: Option<Value>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewFile {
    pub version: u32,
    pub app_version: String,
    pub created_at: String,
    pub view: ViewState,
    pub saved_path: Option<String>,
    pub view_notes: Option<String>,
    pub column_notes: Option<HashMap<String, String>>,
}

pub fn validate_view_file(view_file: &ViewFile) -> Result<(), String> {
    if view_file.version != 1 {
        return Err(format!("Unsupported view version: {}", view_file.version));
    }
    // Puedes agregar más validaciones si hace falta
    Ok(())
}

#[tauri::command]
pub async fn save_view(
    app: tauri::AppHandle,
    _tab_id: String,
    _view_name: String,
    view: ViewState,
    path: Option<String>,
    default_file_name: Option<String>,
    view_notes: Option<String>,
    column_notes: Option<HashMap<String, String>>,
) -> Result<String, String> {
    println!("DEBUG save_view: view_notes='{:?}', column_notes='{:?}'", view_notes, column_notes);

    let final_path = match path {
        Some(p) => p,
        None => {
            use tauri_plugin_dialog::DialogExt;
            let mut dialog = app.dialog().file().add_filter("Exphora View", &["exh"]);
            if let Some(default_name) = default_file_name {
                dialog = dialog.set_file_name(&default_name);
            }
            let result = dialog.blocking_save_file();
            match result {
                Some(p) => p.to_string(),
                None => return Err("Dialog cancelled".to_string()),
            }
        }
    };

    let actual_path = if !final_path.ends_with(".exh") {
        format!("{}.exh", final_path)
    } else {
        final_path
    };

    let created_at = chrono::Utc::now().to_rfc3339();

    let mut view_file = ViewFile {
        version: 1,
        app_version: app.package_info().version.to_string(),
        created_at,
        view,
        saved_path: Some(actual_path.clone()),
        view_notes,
        column_notes,
    };

    let json = serde_json::to_string_pretty(&view_file).map_err(|e| e.to_string())?;
    std::fs::write(&actual_path, json).map_err(|e| e.to_string())?;

    Ok(actual_path)
}

#[tauri::command]
pub async fn load_view(file_path: String) -> Result<ViewFile, String> {
    let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    let view_file: ViewFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    validate_view_file(&view_file)?;

    if !std::path::Path::new(&view_file.view.dataset_path).exists() {
        let err_json = serde_json::json!({
            "code": "DATASET_NOT_FOUND",
            "dataset_path": view_file.view.dataset_path
        });
        return Err(err_json.to_string());
    }

    Ok(view_file)
}

#[tauri::command]
pub async fn relink_view(
    view_file_path: String,
    new_dataset_path: String,
) -> Result<ViewFile, String> {
    let content = std::fs::read_to_string(&view_file_path).map_err(|e| e.to_string())?;
    let mut view_file: ViewFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    view_file.view.dataset_path = new_dataset_path.clone();
    view_file.saved_path = Some(view_file_path.clone());

    if !std::path::Path::new(&new_dataset_path).exists() {
        return Err("New dataset path does not exist".to_string());
    }

    let json = serde_json::to_string_pretty(&view_file).map_err(|e| e.to_string())?;
    std::fs::write(&view_file_path, json).map_err(|e| e.to_string())?;

    Ok(view_file)
}
