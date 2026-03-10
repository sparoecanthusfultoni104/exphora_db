use crate::expr::{eval_expr, ExprValue};
use crate::filters::{apply_filters, sort_indices, DynamicFilters};
use crate::models::{
    compute_stats, record_all_fields, record_title, val_to_str, AppConfig, JsonRecord,
    LoadedDataset, SortDir, TabConfig, TabState,
};
use crate::parser::infer_schema;
use crate::ui::{columns, detail, sidebar, stats, table};
use eframe::egui::{self, Color32, RichText, Stroke};
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

pub struct ExphoraApp {
    pub datasets: Vec<LoadedDataset>,
    pub tab_states: Vec<TabState>,
    pub active_tab: usize,
    pub status_msg: String,
    pub error_msg: String,

    // ── Feature 2: Recent files ───────────────────────────────────────────────
    recent_files: Vec<String>,

    // ── Feature 3: Keyboard shortcuts ─────────────────────────────────────────
    focus_search: bool,

    // ── v0.5.3: Light/dark theme ──────────────────────────────────────────────
    dark_mode: bool,
    /// Whether the Ajustes floating panel is open
    show_settings: bool,
    /// Active tab in Ajustes panel: 0 = Datos, 1 = Sesion P2P
    settings_tab: u8,

    // ── P2P ───────────────────────────────────────────────────────────────────
    p2p_cmd_tx: mpsc::Sender<crate::p2p::Command>,
    p2p_share_pending: Option<oneshot::Receiver<Result<String, String>>>,
    p2p_fetch_pending: Option<oneshot::Receiver<Result<Vec<u8>, String>>>,
    p2p_share_result: Option<String>, // Section 1 only
    p2p_fetch_error: Option<String>,  // Section 2 only
    p2p_fetch_in_flight: bool,
    p2p_panel_open: bool,
    p2p_selected_dataset: usize,
    p2p_port: u16,
    p2p_link_input: String,
    p2p_nat_pending: Option<oneshot::Receiver<Result<std::net::SocketAddr, String>>>,
    p2p_nat_result: Option<Result<std::net::SocketAddr, String>>,
}

impl ExphoraApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        cmd_tx: mpsc::Sender<crate::p2p::Command>,
    ) -> Self {
        let dark_mode = Self::load_prefs_dark_mode();
        apply_theme(&cc.egui_ctx, dark_mode);
        let mut app = Self {
            datasets: Vec::new(),
            tab_states: Vec::new(),
            active_tab: 0,
            status_msg: String::new(),
            error_msg: String::new(),
            recent_files: Self::load_recent_static(),
            focus_search: false,
            dark_mode,
            show_settings: false,
            settings_tab: 0,

            p2p_cmd_tx: cmd_tx,
            p2p_share_pending: None,
            p2p_fetch_pending: None,
            p2p_share_result: None,
            p2p_fetch_error: None,
            p2p_fetch_in_flight: false,
            p2p_panel_open: false,
            p2p_selected_dataset: 0,
            p2p_port: 7878,
            p2p_link_input: String::new(),
            p2p_nat_pending: None,
            p2p_nat_result: None,
        };
        app.load_config();
        app
    }

    fn prefs_path() -> std::path::PathBuf {
        crate::config::app_data_dir().join("prefs.json")
    }

    fn load_prefs_dark_mode() -> bool {
        let path = Self::prefs_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("dark_mode").and_then(|b| b.as_bool()))
            .unwrap_or(true) // default: dark
    }

    fn save_prefs(&self) {
        let dir = crate::config::app_data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let json = serde_json::json!({ "dark_mode": self.dark_mode });
        if let Ok(s) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(Self::prefs_path(), s);
        }
    }

    fn config_path() -> std::path::PathBuf {
        let mut p = std::env::current_exe().unwrap_or_default();
        p.pop();
        p.push("config.json");
        p
    }

    fn load_config(&mut self) {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                for tab_cfg in &config.tabs {
                    if let Err(e) = self.load_file_path(&tab_cfg.path, Some(tab_cfg.name.clone())) {
                        self.error_msg = format!("Error recargando {}: {e}", tab_cfg.name);
                    }
                }
                if !self.datasets.is_empty() {
                    self.active_tab = config.active_tab.min(self.datasets.len() - 1);
                }
            }
        }
    }

    fn save_config(&self) {
        let config = AppConfig {
            tabs: self
                .datasets
                .iter()
                .map(|d| TabConfig {
                    name: d.name.clone(),
                    path: d.path.clone(),
                })
                .collect(),
            active_tab: self.active_tab,
        };
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(Self::config_path(), json);
        }
    }

    // ── Feature 2: Recent files helpers ──────────────────────────────────────

    fn recent_path() -> std::path::PathBuf {
        crate::config::app_data_dir().join("recent.json")
    }

    /// Called from new() — no &self available yet, so it's a static function.
    fn load_recent_static() -> Vec<String> {
        let path = Self::recent_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default()
    }

    fn save_recent(&self) {
        let dir = crate::config::app_data_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(&self.recent_files) {
            let _ = std::fs::write(Self::recent_path(), json);
        }
    }

    fn push_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(10);
        self.save_recent();
    }

    // ── Feature 3: Extracted methods for keyboard shortcuts ───────────────────

    fn close_active_tab(&mut self) {
        self.close_tab(self.active_tab);
    }

    fn export_current_tab_csv(&mut self) {
        self.export_to_csv();
    }

    fn push_tab(&mut self, name: String, path: String, records: Vec<crate::models::JsonRecord>) {
        let n = records.len();
        let schema = infer_schema(&records);
        let mut tab_state = TabState::default();
        tab_state.init_visible_columns(&schema);
        tab_state.sample_col_widths(&schema, &records, 100);
        tab_state.filtered_indices = (0..n).collect();
        self.datasets.push(LoadedDataset {
            name,
            path,
            records,
            schema,
        });
        self.tab_states.push(tab_state);
        self.active_tab = self.datasets.len() - 1;
    }

    // ── Calculated column helpers ───────────────────────────────────────────────

    fn record_to_str_map(record: &JsonRecord) -> HashMap<String, String> {
        record
            .as_object()
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), val_to_str(v)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Recompute calc col cache if dirty.  Call before show_table each frame.
    fn rebuild_calc_cache(ts: &mut TabState, records: &[JsonRecord]) {
        if !ts.calc_col_dirty {
            return;
        }
        ts.calc_col_cache.clear();
        let capacity = records.len();
        for (col_name, expr) in &ts.calculated_cols {
            let mut vals: Vec<Option<String>> = Vec::with_capacity(capacity);
            for record in records {
                let map = Self::record_to_str_map(record);
                let sv = match eval_expr(expr, &map) {
                    ExprValue::Num(n) => {
                        if n.fract() == 0.0 && n.abs() < 1e15 {
                            Some(format!("{}", n as i64))
                        } else {
                            Some(format!("{n}"))
                        }
                    }
                    ExprValue::Str(s) => Some(s),
                    ExprValue::Null => None,
                };
                vals.push(sv);
            }
            ts.calc_col_cache.insert(col_name.clone(), vals);
        }
        ts.calc_col_dirty = false;
    }

    /// Build enriched records for export (injects calc col values as JSON fields).
    fn enrich_records(
        records: &[JsonRecord],
        indices: &[usize],
        calc_cache: &HashMap<String, Vec<Option<String>>>,
    ) -> Vec<JsonRecord> {
        indices
            .iter()
            .filter_map(|&i| {
                let rec = records.get(i)?;
                let mut obj = rec.as_object().cloned().unwrap_or_default();
                for (col, vals) in calc_cache {
                    let v = vals.get(i).and_then(|s| s.as_deref());
                    match v {
                        Some(s) => {
                            obj.insert(col.clone(), serde_json::Value::String(s.to_string()));
                        }
                        None => {
                            obj.insert(col.clone(), serde_json::Value::Null);
                        }
                    }
                }
                Some(serde_json::Value::Object(obj))
            })
            .collect()
    }

    fn load_file_path(&mut self, path: &str, name: Option<String>) -> Result<(), String> {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            // ── Single-tab formats ────────────────────────────────────────────
            "json" => {
                let bytes = std::fs::read(path).map_err(|e| format!("Error leyendo: {e}"))?;
                let records: Vec<crate::models::JsonRecord> = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Error parseando JSON: {e}"))?;
                let n = records.len();
                let tab_name = name.unwrap_or_else(|| tab_name_from_path(path));
                self.push_tab(tab_name, path.to_string(), records);
                self.status_msg = format!("{n} registros cargados correctamente");
                self.push_recent(path);
            }
            "csv" => {
                let bytes = std::fs::read(path).map_err(|e| format!("Error leyendo: {e}"))?;
                let records = crate::parser::parse_csv(&bytes)?;
                let n = records.len();
                let tab_name = name.unwrap_or_else(|| tab_name_from_path(path));
                self.push_tab(tab_name, path.to_string(), records);
                self.status_msg = format!("{n} registros cargados correctamente");
                self.push_recent(path);
            }
            "ndjson" | "jsonl" => {
                let bytes = std::fs::read(path).map_err(|e| format!("Error leyendo: {e}"))?;
                let records = crate::parser::parse_ndjson(&bytes)?;
                let n = records.len();
                let tab_name = name.unwrap_or_else(|| tab_name_from_path(path));
                self.push_tab(tab_name, path.to_string(), records);
                self.status_msg = format!("{n} registros cargados correctamente");
                self.push_recent(path);
            }
            "xml" => {
                let bytes = std::fs::read(path).map_err(|e| format!("Error leyendo: {e}"))?;
                let records = crate::parser::parse_xml(&bytes)?;
                let n = records.len();
                let tab_name = name.unwrap_or_else(|| tab_name_from_path(path));
                self.push_tab(tab_name, path.to_string(), records);
                self.status_msg = format!("{n} registros cargados correctamente");
                self.push_recent(path);
            }
            // ── Multi-tab: SQLite ─────────────────────────────────────────────
            "db" | "sqlite" | "sqlite3" => {
                let tables = crate::parser::parse_sqlite(std::path::Path::new(path))?;
                if tables.is_empty() {
                    return Err("El archivo SQLite no contiene tablas de usuario.".to_string());
                }
                let total_tables = tables.len();
                let mut total_records = 0usize;
                for (table_name, records) in tables {
                    let n = records.len();
                    total_records += n;
                    self.push_tab(table_name, path.to_string(), records);
                }
                self.status_msg = format!(
                    "{total_tables} tablas / {total_records} registros cargados correctamente"
                );
                self.push_recent(path);
            }
            _ => {
                return Err(
                    "Formato no soportado. Usa .json .csv .ndjson .jsonl .db .sqlite .xml"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    fn open_file_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Datos (*.json *.csv *.ndjson *.jsonl *.db *.sqlite *.xml)",
                &[
                    "json", "csv", "ndjson", "jsonl", "db", "sqlite", "sqlite3", "xml",
                ],
            )
            .pick_file()
        else {
            return;
        };
        self.error_msg.clear();
        let path_str = path.display().to_string();
        if let Err(e) = self.load_file_path(&path_str, None) {
            self.error_msg = e;
        }
    }

    fn recompute_filter(&mut self) {
        if self.active_tab >= self.datasets.len() {
            return;
        }
        let ds = &self.datasets[self.active_tab];
        let ts = &mut self.tab_states[self.active_tab];

        let filters = DynamicFilters {
            text_search: ts.text_search.clone(),
            filters: ts.filters.clone(),
            easy_filters: ts.easy_filters.clone(),
            filter_mode: ts.filter_mode.clone(),
        };
        ts.filtered_indices = apply_filters(&filters, &ds.schema, &ds.records);
        ts.selected_row = None;

        if let Some(ref col) = ts.sort_column {
            let ascending = ts.sort_dir == SortDir::Asc;
            sort_indices(&mut ts.filtered_indices, &ds.records, col, ascending);
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if idx < self.datasets.len() {
            self.datasets.remove(idx);
            self.tab_states.remove(idx);
            if self.active_tab >= self.datasets.len() && self.active_tab > 0 {
                self.active_tab -= 1;
            }
        }
    }

    // ── Feature 1: Export filtered+visible data to CSV ───────────────────────────────────
    fn export_to_csv(&mut self) {
        if self.active_tab >= self.datasets.len() {
            return;
        }
        let ds = &self.datasets[self.active_tab];
        let ts = &self.tab_states[self.active_tab];

        // vis_cols includes calc cols (appended by get_all_visible_cols)
        let schema_cols = ts.get_visible_columns(&ds.schema);
        let calc_names: Vec<String> = ts.calculated_cols.iter().map(|(n, _)| n.clone()).collect();
        let vis_cols: Vec<String> = schema_cols.into_iter().chain(calc_names).collect();

        let enriched = Self::enrich_records(&ds.records, &ts.filtered_indices, &ts.calc_col_cache);
        let tab_name = ds.name.clone();

        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{tab_name}.csv"))
            .add_filter("CSV", &["csv"])
            .save_file()
        else {
            return;
        };

        let mut wtr = String::new();
        let header: Vec<String> = vis_cols
            .iter()
            .map(|col| {
                if col.contains(',') || col.contains('"') {
                    format!("\"{}\"", col.replace('"', "\"\""))
                } else {
                    col.clone()
                }
            })
            .collect();
        wtr.push_str(&header.join(","));
        wtr.push('\n');

        let exported = enriched.len();
        for record in &enriched {
            let obj = record.as_object();
            let row: Vec<String> = vis_cols
                .iter()
                .map(|col| {
                    let val = obj
                        .and_then(|o| o.get(col))
                        .map(|v| val_to_str(v))
                        .unwrap_or_default();
                    if val.contains(',') || val.contains('"') || val.contains('\n') {
                        format!("\"{}\"", val.replace('"', "\"\""))
                    } else {
                        val
                    }
                })
                .collect();
            wtr.push_str(&row.join(","));
            wtr.push('\n');
        }

        match std::fs::write(&path, wtr) {
            Ok(_) => {
                self.status_msg = format!("Exportado: {exported} registros -> {}", path.display());
                self.error_msg.clear();
            }
            Err(e) => {
                self.error_msg = format!("Error exportando CSV: {e}");
            }
        }
    }

    /// Export the active tab in the given format.
    /// `fmt` is one of: "json", "xlsx", "md", "pdf"
    fn export_format(&mut self, fmt: &str) {
        if self.active_tab >= self.datasets.len() {
            return;
        }
        let ds = &self.datasets[self.active_tab];
        let ts = &self.tab_states[self.active_tab];

        let schema_cols = ts.get_visible_columns(&ds.schema);
        let calc_names: Vec<String> = ts.calculated_cols.iter().map(|(n, _)| n.clone()).collect();
        let vis_cols: Vec<String> = schema_cols.into_iter().chain(calc_names).collect();

        let enriched = Self::enrich_records(&ds.records, &ts.filtered_indices, &ts.calc_col_cache);
        let refs: Vec<&JsonRecord> = enriched.iter().collect();
        let tab_name = ds.name.clone();

        let (filter_name, extensions, default_name) = match fmt {
            "json" => ("JSON", vec!["json"], format!("{tab_name}.json")),
            "xlsx" => ("Excel", vec!["xlsx"], format!("{tab_name}.xlsx")),
            "md" => ("Markdown", vec!["md"], format!("{tab_name}.md")),
            "pdf" => ("PDF", vec!["pdf"], format!("{tab_name}.pdf")),
            _ => return,
        };

        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(filter_name, &extensions)
            .save_file()
        else {
            return;
        };

        let result = match fmt {
            "json" => crate::parser::export_to_json(&refs, &vis_cols, &path),
            "xlsx" => crate::parser::export_to_xlsx(&refs, &vis_cols, &path),
            "md" => crate::parser::export_to_markdown(&refs, &vis_cols, &path),
            "pdf" => crate::parser::export_to_pdf(&refs, &vis_cols, &path, &tab_name),
            _ => return,
        };

        match result {
            Ok(()) => {
                self.status_msg =
                    format!("Exportado: {} registros -> {}", refs.len(), path.display());
                self.error_msg.clear();
            }
            Err(e) => {
                self.error_msg = format!("Error exportando {fmt}: {e}");
            }
        }
    }

    // ── load_from_bytes: insert a P2P-received dataset as a new tab ──────────
    fn load_from_bytes(&mut self, name: String, bytes: Vec<u8>) -> Result<(), String> {
        let records: Vec<JsonRecord> =
            serde_json::from_slice(&bytes).map_err(|e| format!("JSON inválido: {e}"))?;
        let n = records.len();
        let schema = infer_schema(&records);
        let mut tab_state = TabState::default();
        tab_state.init_visible_columns(&schema);
        tab_state.filtered_indices = (0..n).collect();
        self.datasets.push(LoadedDataset {
            name,
            path: String::new(),
            records,
            schema,
        });
        self.tab_states.push(tab_state);
        self.active_tab = self.datasets.len() - 1;
        self.status_msg = format!("{n} registros recibidos vía P2P");
        Ok(())
    }
}

impl eframe::App for ExphoraApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Fix: apply theme every frame so eframe can't reset it ──────────────────────
        apply_theme(ctx, self.dark_mode);

        // ── Feature 3: Keyboard shortcuts (before any panel) ────────────────────────
        let mut open_dialog = false;
        // Check whether any text widget has keyboard focus BEFORE reading input,
        // so we can correctly guard the Escape shortcut.
        let some_widget_focused = ctx.memory(|m| m.focused().is_some());
        ctx.input(|i| {
            // Ctrl+O → open file dialog
            if i.modifiers.ctrl && i.key_pressed(egui::Key::O) {
                open_dialog = true;
            }
            // Ctrl+W → close active tab
            if i.modifiers.ctrl && i.key_pressed(egui::Key::W) {
                if !self.datasets.is_empty() {
                    self.close_active_tab();
                }
            }
            // Ctrl+E → export CSV of active tab (guard: datasets must exist)
            if i.modifiers.ctrl && i.key_pressed(egui::Key::E) {
                if !self.datasets.is_empty() {
                    self.export_current_tab_csv();
                }
            }
            // Escape → clear filters, but only when no text widget has keyboard focus
            // so the user can still press Escape inside a TextEdit without losing filters
            if i.key_pressed(egui::Key::Escape) && !some_widget_focused {
                if let Some(ts) = self.tab_states.get_mut(self.active_tab) {
                    ts.filters.clear();
                    ts.text_search.clear();
                    ts.filtered_indices = (0..self
                        .datasets
                        .get(self.active_tab)
                        .map(|d| d.records.len())
                        .unwrap_or(0))
                        .collect();
                }
            }
            // Ctrl+F → focus search bar
            if i.modifiers.ctrl && i.key_pressed(egui::Key::F) {
                self.focus_search = true;
            }
        });
        // Resolve Ctrl+O outside ctx.input to avoid double-borrow
        if open_dialog {
            self.open_file_dialog();
        }

        // ── P2P polling (non-blocking, every frame) ───────────────────────────
        if let Some(rx) = &mut self.p2p_share_pending {
            match rx.try_recv() {
                Ok(Ok(link_str)) => {
                    self.p2p_share_result = Some(link_str);
                    self.p2p_share_pending = None;
                }
                Ok(Err(e)) => {
                    self.p2p_share_result = Some(format!("Error: {e}"));
                    self.p2p_share_pending = None;
                }
                Err(_) => {} // still in progress
            }
        }
        if let Some(rx) = &mut self.p2p_fetch_pending {
            match rx.try_recv() {
                Ok(Ok(bytes)) => {
                    self.p2p_fetch_in_flight = false;
                    self.p2p_fetch_pending = None;
                    if let Err(e) = self.load_from_bytes("P2P Dataset".to_string(), bytes) {
                        self.p2p_fetch_error = Some(format!("Error al cargar: {e}"));
                    }
                }
                Ok(Err(e)) => {
                    self.p2p_fetch_in_flight = false;
                    self.p2p_fetch_pending = None;
                    self.p2p_fetch_error = Some(format!("Error: {e}"));
                }
                Err(_) => {} // still in progress
            }
        }
        if let Some(rx) = &mut self.p2p_nat_pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.p2p_nat_result = Some(result);
                    self.p2p_nat_pending = None;
                }
                Err(_) => {}
            }
        }
        if self.p2p_share_pending.is_some()
            || self.p2p_fetch_pending.is_some()
            || self.p2p_nat_pending.is_some()
        {
            ctx.request_repaint();
        }

        // ── Top bar ───────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("topbar")
            .exact_height(52.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 24, 40))
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("ExphoraDB")
                            .size(18.0)
                            .strong()
                            .color(Color32::from_rgb(130, 190, 255)),
                    );
                    ui.add_space(16.0);

                    let mut tab_to_close: Option<usize> = None;
                    let mut tab_to_activate: Option<usize> = None;

                    for i in 0..self.datasets.len() {
                        let is_active = i == self.active_tab;
                        let ds = &self.datasets[i];

                        let fill = if is_active {
                            Color32::from_rgb(40, 100, 180)
                        } else {
                            Color32::from_rgb(25, 33, 55)
                        };
                        let text_col = if is_active {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(140, 160, 200)
                        };

                        ui.horizontal(|ui| {
                            let btn = ui.add(
                                egui::Button::new(
                                    RichText::new(&ds.name).color(text_col).size(12.0),
                                )
                                .fill(fill)
                                .stroke(if is_active {
                                    Stroke::new(1.0, Color32::from_rgb(80, 150, 255))
                                } else {
                                    Stroke::NONE
                                }),
                            );
                            if btn.clicked() {
                                tab_to_activate = Some(i);
                            }

                            let x_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("x")
                                        .color(Color32::from_rgb(180, 100, 100))
                                        .size(10.0),
                                )
                                .fill(Color32::TRANSPARENT)
                                .frame(false),
                            );
                            if x_btn.clicked() {
                                tab_to_close = Some(i);
                            }
                        });
                        ui.add_space(2.0);
                    }

                    // "+" button
                    let plus_btn = ui.add(
                        egui::Button::new(RichText::new("+").color(Color32::WHITE).size(14.0))
                            .fill(Color32::from_rgb(50, 140, 80))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(80, 200, 120))),
                    );
                    if plus_btn.clicked() {
                        self.open_file_dialog();
                    }

                    // "Recientes ▾" dropdown button (Feature 2)
                    ui.add_space(4.0);
                    let rec_id = ui.make_persistent_id("recientes_popup");
                    let rec_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Recientes ▾")
                                .color(Color32::from_rgb(180, 200, 240))
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(28, 38, 65))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(60, 80, 130))),
                    );
                    if rec_btn.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(rec_id));
                    }
                    egui::popup_below_widget(
                        ui,
                        rec_id,
                        &rec_btn,
                        egui::PopupCloseBehavior::CloseOnClickOutside,
                        |ui| {
                            ui.set_min_width(280.0);
                            if self.recent_files.is_empty() {
                                ui.label(
                                    RichText::new("Sin archivos recientes")
                                        .color(Color32::from_rgb(100, 110, 140))
                                        .size(12.0),
                                );
                            } else {
                                let mut load_path: Option<String> = None;
                                for path in &self.recent_files {
                                    let exists = std::path::Path::new(path).exists();
                                    let label_color = if exists {
                                        Color32::from_rgb(200, 215, 245)
                                    } else {
                                        Color32::from_rgb(90, 100, 130)
                                    };
                                    let resp = ui.add(
                                        egui::Button::new(
                                            RichText::new(path).color(label_color).size(11.0),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .frame(false),
                                    );
                                    if !exists {
                                        resp.on_hover_text("Archivo no encontrado");
                                    } else if resp.clicked() {
                                        load_path = Some(path.clone());
                                        ui.memory_mut(|m| m.close_popup());
                                    }
                                }
                                if let Some(p) = load_path {
                                    self.error_msg.clear();
                                    if let Err(e) = self.load_file_path(&p, None) {
                                        self.error_msg = e;
                                    }
                                }
                                ui.separator();
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Limpiar historial")
                                                .color(Color32::from_rgb(200, 100, 100))
                                                .size(11.0),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    self.recent_files.clear();
                                    self.save_recent();
                                    ui.memory_mut(|m| m.close_popup());
                                }
                            }
                        },
                    );

                    if let Some(idx) = tab_to_activate {
                        self.active_tab = idx;
                    }
                    if let Some(idx) = tab_to_close {
                        self.close_tab(idx);
                    }

                    ui.add_space(12.0);

                    // ── [Ajustes] button (replaces scattered buttons) ────────────────────────────
                    let ajustes_on = self.show_settings;
                    let ajustes_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Ajustes")
                                .color(if ajustes_on {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(180, 200, 240)
                                })
                                .size(12.0),
                        )
                        .fill(if ajustes_on {
                            Color32::from_rgb(50, 80, 150)
                        } else {
                            Color32::from_rgb(28, 38, 65)
                        })
                        .stroke(Stroke::new(1.0, Color32::from_rgb(60, 80, 130))),
                    );
                    if ajustes_btn.clicked() {
                        self.show_settings = !ajustes_on;
                    }

                    ui.add_space(8.0);

                    if !self.error_msg.is_empty() {
                        ui.label(
                            RichText::new(&self.error_msg)
                                .color(Color32::from_rgb(255, 100, 100))
                                .size(11.0),
                        );
                    } else if !self.status_msg.is_empty() {
                        ui.label(
                            RichText::new(&self.status_msg)
                                .color(Color32::from_rgb(100, 220, 130))
                                .size(11.0),
                        );
                    }
                });
            });

        // ── Panel de Ajustes (floating window with tabs) ───────────────────────
        if self.show_settings {
            let mut settings_open = self.show_settings;
            egui::Window::new("Ajustes")
                .id(egui::Id::new("ajustes_panel"))
                .resizable(false)
                .collapsible(false)
                .min_width(260.0)
                .open(&mut settings_open)
                .show(ctx, |ui| {
                    let has_data = !self.datasets.is_empty();

                    // ── Tab bar ─────────────────────────────────────────────────────
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.settings_tab,
                            0u8,
                            RichText::new("Datos").size(12.0),
                        );
                        ui.selectable_value(
                            &mut self.settings_tab,
                            1u8,
                            RichText::new("Sesion P2P").size(12.0),
                        );
                    });
                    ui.separator();

                    match self.settings_tab {
                        // ── TAB 0: Datos ───────────────────────────────────────
                        0 => {
                            ui.add_space(6.0);

                            // + columna calculada
                            let calc_btn = ui.add_enabled(
                                has_data,
                                egui::Button::new(
                                    RichText::new("+ columna calculada")
                                        .color(Color32::from_rgb(180, 255, 200))
                                        .size(12.0),
                                )
                                .fill(Color32::from_rgb(20, 60, 40))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(80, 200, 140))),
                            );
                            if calc_btn.clicked() {
                                let ts = &mut self.tab_states[self.active_tab];
                                if ts.calc_col_editor.is_none() {
                                    ts.calc_col_editor = Some((String::new(), String::new()));
                                }
                            }

                            ui.add_space(6.0);

                            // Exportar dropdown
                            let exp_id = ui.make_persistent_id("ajustes_exportar_popup");
                            let exp_btn = ui.add_enabled(
                                has_data,
                                egui::Button::new(
                                    RichText::new("Exportar ▾")
                                        .color(Color32::from_rgb(140, 220, 160))
                                        .size(12.0),
                                )
                                .fill(Color32::from_rgb(30, 60, 40))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(60, 160, 90))),
                            );
                            if exp_btn.clicked() {
                                ui.memory_mut(|m| m.toggle_popup(exp_id));
                            }
                            egui::popup_below_widget(
                                ui,
                                exp_id,
                                &exp_btn,
                                egui::PopupCloseBehavior::CloseOnClickOutside,
                                |ui| {
                                    ui.set_min_width(120.0);
                                    if ui.button("JSON").clicked() {
                                        ui.memory_mut(|m| m.close_popup());
                                        self.export_format("json");
                                    }
                                    if ui.button("CSV").clicked() {
                                        ui.memory_mut(|m| m.close_popup());
                                        self.export_to_csv();
                                    }
                                    if ui.button("Excel").clicked() {
                                        ui.memory_mut(|m| m.close_popup());
                                        self.export_format("xlsx");
                                    }
                                    if ui.button("Markdown").clicked() {
                                        ui.memory_mut(|m| m.close_popup());
                                        self.export_format("md");
                                    }
                                    if ui.button("PDF").clicked() {
                                        ui.memory_mut(|m| m.close_popup());
                                        self.export_format("pdf");
                                    }
                                },
                            );
                        }

                        // ── TAB 1: Sesion P2P ─────────────────────────────────
                        _ => {
                            ui.add_space(6.0);
                            let p2p_on = self.p2p_panel_open;
                            let p2p_label = if p2p_on {
                                "Abrir panel P2P (activo)"
                            } else {
                                "Abrir panel P2P"
                            };
                            let p2p_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(p2p_label)
                                        .color(if p2p_on {
                                            Color32::WHITE
                                        } else {
                                            Color32::from_rgb(255, 180, 80)
                                        })
                                        .size(12.0),
                                )
                                .fill(if p2p_on {
                                    Color32::from_rgb(140, 80, 20)
                                } else {
                                    Color32::from_rgb(25, 33, 55)
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    if p2p_on {
                                        Color32::from_rgb(220, 140, 60)
                                    } else {
                                        Color32::from_rgb(100, 80, 40)
                                    },
                                )),
                            );
                            if p2p_btn.clicked() {
                                self.p2p_panel_open = !p2p_on;
                                if !p2p_on
                                    && self.p2p_nat_result.is_none()
                                    && self.p2p_nat_pending.is_none()
                                {
                                    let (resp_tx, resp_rx) = oneshot::channel();
                                    let _ =
                                        self.p2p_cmd_tx.try_send(crate::p2p::Command::DetectNat {
                                            port: self.p2p_port,
                                            resp: resp_tx,
                                        });
                                    self.p2p_nat_pending = Some(resp_rx);
                                }
                            }
                        }
                    }
                });
            self.show_settings = settings_open;
        }

        // ── Bottom status bar ─────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(28.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(15, 20, 35))
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.active_tab < self.datasets.len() {
                        let ds = &self.datasets[self.active_tab];
                        let ts = &self.tab_states[self.active_tab];
                        let total = ds.records.len();
                        let found = ts.filtered_indices.len();
                        let vis_count =
                            ts.visible_columns.values().filter(|&&v| v).count();
                        let active_filters = ts.filters.len();
                        ui.label(
                            RichText::new(format!(
                                "{found} / {total}  |  campos: {} ({vis_count} vis)  |  filtros: {active_filters}  |  {}",
                                ds.schema.fields.len(),
                                ds.path
                            ))
                            .color(Color32::from_rgb(100, 140, 200))
                            .size(12.0),
                        );
                    } else {
                        ui.label(
                            RichText::new("Usa el boton + para cargar un archivo JSON")
                                .color(Color32::from_rgb(80, 110, 160))
                                .size(12.0),
                        );
                    }
                });
            });

        // ── Guard: no tabs ────────────────────────────────────────────────────
        if self.datasets.is_empty() {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(18, 24, 38))
                        .inner_margin(egui::Margin::symmetric(40, 40)),
                )
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(100.0);
                        ui.label(
                            RichText::new("ExphoraDB")
                                .size(28.0)
                                .strong()
                                .color(Color32::from_rgb(80, 130, 200)),
                        );
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(
                                "Carga un archivo JSON con el boton + en la barra superior",
                            )
                            .size(14.0)
                            .color(Color32::from_rgb(100, 130, 180)),
                        );
                    });
                });
            return;
        }

        // ── Left sidebar: text search + field dropdowns ───────────────────────
        let mut filter_changed = false;
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(220.0)
            .min_width(180.0)
            .max_width(320.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 28, 48))
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("FILTROS")
                        .size(11.0)
                        .color(Color32::from_rgb(80, 100, 150)),
                );
                ui.separator();

                let ds = &self.datasets[self.active_tab];
                let ts = &mut self.tab_states[self.active_tab];

                let mut temp_filters = DynamicFilters {
                    text_search: ts.text_search.clone(),
                    filters: ts.filters.clone(),
                    easy_filters: ts.easy_filters.clone(),
                    filter_mode: ts.filter_mode.clone(),
                };

                if sidebar::show_sidebar(ui, &mut temp_filters, &ds.schema, &ts.visible_columns) {
                    ts.text_search = temp_filters.text_search;
                    ts.filters = temp_filters.filters;
                    ts.easy_filters = temp_filters.easy_filters;
                    ts.filter_mode = temp_filters.filter_mode;
                    filter_changed = true;
                }

                // Feature 3 — Ctrl+F: focus the search TextEdit.
                // The TextEdit is the first auto-ID widget inside the ScrollArea
                // with id_salt "sidebar_scroll". We derive its stable ID the same
                // way egui does: Id::new("sidebar_scroll") hashed-with its first
                // auto counter (0).
                if self.focus_search {
                    let scroll_id = egui::Id::new("sidebar_scroll");
                    // egui::TextEdit auto-ID is: parent_id.with(ui_auto_counter)
                    // We request focus on the scroll area itself; egui routes focus
                    // to the first focusable child on the next frame.
                    ctx.memory_mut(|m| m.request_focus(scroll_id.with(1u64)));
                    self.focus_search = false;
                }
            });

        if filter_changed {
            self.recompute_filter();
        }

        // ── Right panel: columns / stats / detail ────────────────────────────
        let mut columns_changed = false;
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(280.0)
            .min_width(200.0)
            .max_width(450.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 26, 44))
                    .inner_margin(egui::Margin::symmetric(10, 8)),
            )
            .show(ctx, |ui| {
                let ds = &self.datasets[self.active_tab];
                let ts = &mut self.tab_states[self.active_tab];

                if ts.show_columns_panel {
                    if columns::show_columns_panel(ui, &ds.schema, &mut ts.visible_columns) {
                        columns_changed = true;
                        let hidden: Vec<String> = ts
                            .visible_columns
                            .iter()
                            .filter(|(_, &v)| !v)
                            .map(|(k, _)| k.clone())
                            .collect();
                        for field in &hidden {
                            ts.filters.remove(field);
                        }
                    }
                } else if ts.show_stats {
                    stats::show_stats_panel(
                        ui,
                        &ds.records,
                        &ts.filtered_indices,
                        &ds.schema,
                        &mut ts.stats_field,
                    );
                } else {
                    ui.label(
                        RichText::new("DETALLE")
                            .size(11.0)
                            .color(Color32::from_rgb(80, 100, 150)),
                    );
                    ui.separator();
                    ui.add_space(4.0);

                    if let Some(row) = ts.selected_row {
                        if let Some(&record_idx) = ts.filtered_indices.get(row) {
                            if let Some(record) = ds.records.get(record_idx) {
                                let fields = record_all_fields(record);
                                let title = record_title(record);
                                detail::show_detail(ui, &title, &fields);
                            }
                        }
                    } else {
                        detail::show_no_selection(ui);
                    }
                }
            });

        if columns_changed {
            self.recompute_filter();
        }

        // ── P2P panel (right, innermost — declared after right_panel for LIFO) ─
        if self.p2p_panel_open {
            egui::SidePanel::right("p2p_panel")
                .resizable(true)
                .default_width(260.0)
                .min_width(220.0)
                .max_width(360.0)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(20, 28, 48))
                        .inner_margin(egui::Margin::symmetric(10, 8)),
                )
                .show(ctx, |ui| {
                    // ── NAT status block ─────────────────────────────────────
                    ui.label(
                        RichText::new("ESTADO DE RED")
                            .size(11.0)
                            .color(Color32::from_rgb(80, 100, 150)),
                    );
                    ui.separator();

                    match (&self.p2p_nat_pending, &self.p2p_nat_result) {
                        (Some(_), _) => {
                            // Detecting in progress
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Detectando IP publica...")
                                        .color(Color32::from_rgb(180, 200, 230))
                                        .size(11.0),
                                );
                            });
                        }
                        (None, Some(Ok(addr))) if addr.ip() == std::net::Ipv4Addr::UNSPECIFIED => {
                            // STUN returned 0.0.0.0 — treat as failure
                            ui.label(
                                RichText::new(
                                    "NAT simetrico / CGNAT detectado\nComparte via Tailscale o usa port forwarding",
                                )
                                .color(Color32::from_rgb(220, 180, 60))
                                .size(11.0),
                            );
                        }
                        (None, Some(Ok(addr))) => {
                            ui.label(
                                RichText::new(format!("IP publica: {addr}"))
                                    .color(Color32::from_rgb(100, 210, 130))
                                    .size(11.0),
                            );
                        }
                        (None, Some(Err(_))) => {
                            ui.label(
                                RichText::new(
                                    "NAT simetrico / CGNAT detectado\nComparte via Tailscale o usa port forwarding",
                                )
                                .color(Color32::from_rgb(220, 180, 60))
                                .size(11.0),
                            );
                        }
                        (None, None) => {
                            // Not yet initiated — show manual trigger button
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Detectar red")
                                            .color(Color32::from_rgb(140, 160, 200))
                                            .size(11.0),
                                    )
                                    .fill(Color32::from_rgb(25, 33, 55)),
                                )
                                .clicked()
                            {
                                let (resp_tx, resp_rx) = oneshot::channel();
                                let _ = self.p2p_cmd_tx.try_send(
                                    crate::p2p::Command::DetectNat {
                                        port: self.p2p_port,
                                        resp: resp_tx,
                                    },
                                );
                                self.p2p_nat_pending = Some(resp_rx);
                            }
                        }
                    }

                    // Re-detect button — shown only when a result already exists
                    if self.p2p_nat_result.is_some() {
                        ui.add_space(2.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Detectar red")
                                        .color(Color32::from_rgb(140, 160, 200))
                                        .size(11.0),
                                )
                                .fill(Color32::from_rgb(25, 33, 55)),
                            )
                            .clicked()
                        {
                            self.p2p_nat_result = None;
                            let (resp_tx, resp_rx) = oneshot::channel();
                            let _ = self.p2p_cmd_tx.try_send(
                                crate::p2p::Command::DetectNat {
                                    port: self.p2p_port,
                                    resp: resp_tx,
                                },
                            );
                            self.p2p_nat_pending = Some(resp_rx);
                        }
                    }

                    ui.add_space(6.0);
                    ui.separator();

                    // ── Section 1: Compartir ─────────────────────────────────
                    ui.label(
                        RichText::new("COMPARTIR DATASET")
                            .size(11.0)
                            .color(Color32::from_rgb(80, 100, 150)),
                    );
                    ui.separator();

                    let has_datasets = !self.datasets.is_empty();
                    let sharing = self.p2p_share_pending.is_some();

                    ui.add_enabled_ui(has_datasets && !sharing, |ui| {
                        let selected_name = self
                            .datasets
                            .get(self.p2p_selected_dataset)
                            .map(|d| d.name.as_str())
                            .unwrap_or("--");
                        egui::ComboBox::from_id_salt("p2p_dataset_select")
                            .selected_text(selected_name)
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                for (i, ds) in self.datasets.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.p2p_selected_dataset,
                                        i,
                                        &ds.name,
                                    );
                                }
                            });
                        let port_before = self.p2p_port;
                        ui.add(
                            egui::DragValue::new(&mut self.p2p_port)
                                .range(1024u16..=65535)
                                .prefix("Puerto: "),
                        );
                        if self.p2p_port != port_before {
                            // Port changed — invalidate cached NAT result so it
                            // re-detects with the new port on the next trigger.
                            self.p2p_nat_result = None;
                        }
                    });

                    ui.add_space(4.0);
                    let share_btn = ui.add_enabled(
                        has_datasets && !sharing,
                        egui::Button::new(
                            RichText::new("Compartir")
                                .color(Color32::from_rgb(140, 220, 160))
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(30, 60, 40)),
                    );
                    if share_btn.clicked() {
                        self.p2p_share_result = None; // clear previous
                        if let Some(ds) = self.datasets.get(self.p2p_selected_dataset) {
                            let json_bytes = serde_json::to_vec(&ds.records).unwrap_or_default();
                            let (resp_tx, resp_rx) = oneshot::channel();
                            match self.p2p_cmd_tx.try_send(crate::p2p::Command::ShareDataset {
                                name: ds.name.clone(),
                                json_bytes,
                                port: self.p2p_port,
                                resp: resp_tx,
                            }) {
                                Ok(()) => {
                                    self.p2p_share_pending = Some(resp_rx);
                                }
                                Err(_) => {
                                    self.p2p_share_result =
                                        Some("Error: P2P ocupado, reintenta.".into());
                                }
                            }
                        }
                    }

                    if sharing {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Comprimiendo y generando link...")
                                    .color(Color32::from_rgb(180, 200, 230))
                                    .size(11.0),
                            );
                        });
                    }

                    if let Some(result) = self.p2p_share_result.clone() {
                        ui.add_space(4.0);
                        if result.starts_with("Error") {
                            ui.label(
                                RichText::new(&result)
                                    .color(Color32::from_rgb(255, 100, 100))
                                    .size(11.0),
                            );
                        } else {
                            let mut link_display = result.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut link_display)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY),
                            );
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("Copiar").size(12.0))
                                        .fill(Color32::from_rgb(30, 50, 80)),
                                )
                                .clicked()
                            {
                                ctx.copy_text(result.clone());
                            }
                        }
                    }

                    ui.add_space(12.0);
                    ui.separator();

                    // ── Section 2: Recibir ───────────────────────────────────
                    ui.label(
                        RichText::new("RECIBIR DATASET")
                            .size(11.0)
                            .color(Color32::from_rgb(80, 100, 150)),
                    );
                    ui.separator();

                    let fetching = self.p2p_fetch_in_flight;
                    ui.add_enabled_ui(!fetching, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.p2p_link_input)
                                .hint_text("Pega el link  exphora:...")
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );
                    });

                    ui.add_space(4.0);
                    let fetch_btn = ui.add_enabled(
                        !fetching && !self.p2p_link_input.trim().is_empty(),
                        egui::Button::new(
                            RichText::new("Descargar")
                                .color(Color32::from_rgb(140, 200, 255))
                                .size(12.0),
                        )
                        .fill(Color32::from_rgb(25, 50, 85)),
                    );
                    if fetch_btn.clicked() {
                        self.p2p_fetch_error = None; // clear previous
                        let (resp_tx, resp_rx) = oneshot::channel();
                        match self.p2p_cmd_tx.try_send(crate::p2p::Command::FetchDataset {
                            link: self.p2p_link_input.trim().to_string(),
                            resp: resp_tx,
                        }) {
                            Ok(()) => {
                                self.p2p_fetch_pending = Some(resp_rx);
                                self.p2p_fetch_in_flight = true;
                            }
                            Err(_) => {
                                self.p2p_fetch_error = Some("P2P ocupado, reintenta.".into());
                            }
                        }
                    }

                    if fetching {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new("Descargando...")
                                    .color(Color32::from_rgb(180, 200, 230))
                                    .size(11.0),
                            );
                        });
                    }

                    if let Some(err) = &self.p2p_fetch_error {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(err)
                                .color(Color32::from_rgb(255, 100, 100))
                                .size(11.0),
                        );
                    }
                });
        }

        // ── Central panel: table ─────────────────────────────────────────────
        let mut sort_changed = false;
        let mut panel_filter_changed = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 24, 38))
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                let tab_idx = self.active_tab;
                let ds = &self.datasets[tab_idx];

                // Rebuild calc cache only when dirty (never every frame unless changed)
                Self::rebuild_calc_cache(&mut self.tab_states[tab_idx], &ds.records);

                let ts = &mut self.tab_states[tab_idx];

                // vis_cols = schema-visible + calculated columns
                let schema_cols = ts.get_visible_columns(&ds.schema);
                let calc_names: Vec<String> =
                    ts.calculated_cols.iter().map(|(n, _)| n.clone()).collect();
                let vis_cols: Vec<String> = schema_cols
                    .into_iter()
                    .chain(calc_names.iter().cloned())
                    .collect();

                let frozen_cols_clone = ts.frozen_cols.clone();
                let calc_col_cache_ref = &ts.calc_col_cache;
                let col_widths_ref = &ts.col_widths;
                let sort_col_ref = ts.sort_column.as_deref();
                let sort_asc = ts.sort_dir == SortDir::Asc;
                let search_text = ts.text_search.clone();

                let action = table::show_table(
                    ui,
                    &ds.records,
                    &ts.filtered_indices,
                    &vis_cols,
                    &frozen_cols_clone,
                    &calc_names,
                    calc_col_cache_ref,
                    col_widths_ref,
                    sort_col_ref,
                    sort_asc,
                    ts.selected_row,
                    &search_text,
                    &ds.schema,
                    &mut ts.open_filter_panel,
                    &ts.filters,
                    tab_idx,
                );

                match action {
                    table::TableAction::ClickRow(row) => {
                        ts.selected_row = if ts.selected_row == Some(row) {
                            None
                        } else {
                            Some(row)
                        };
                    }
                    table::TableAction::ClickHeader(col_name) => {
                        if ts.sort_column.as_deref() == Some(col_name.as_str()) {
                            ts.sort_dir = match ts.sort_dir {
                                SortDir::Asc => SortDir::Desc,
                                SortDir::Desc => SortDir::Asc,
                            };
                        } else {
                            ts.sort_column = Some(col_name);
                            ts.sort_dir = SortDir::Asc;
                        }
                        sort_changed = true;
                    }
                    table::TableAction::ShowColStats(col_name) => {
                        let stats = compute_stats(&ds.records, &ts.filtered_indices, &col_name);
                        ts.active_stats = Some((col_name, stats));
                    }
                    table::TableAction::ToggleFreeze(col_name) => {
                        if let Some(pos) = ts.frozen_cols.iter().position(|c| c == &col_name) {
                            ts.frozen_cols.remove(pos);
                        } else if ts.frozen_cols.len() < 5 {
                            ts.frozen_cols.push(col_name);
                        }
                    }
                    table::TableAction::RemoveCalcCol(col_name) => {
                        ts.calculated_cols.retain(|(n, _)| n != &col_name);
                        ts.calc_col_cache.remove(&col_name);
                        ts.frozen_cols.retain(|c| c != &col_name);
                        ts.calc_col_dirty = true;
                    }
                    table::TableAction::None => {}
                }

                // ── Floating column stats window ──────────────────────────────
                if let Some((col_name, stats)) = ts.active_stats.clone() {
                    let mut stats_open = true;
                    egui::Window::new(format!("Estadisticas -- {col_name}"))
                        .id(egui::Id::new("col_stats").with(tab_idx))
                        .resizable(false)
                        .collapsible(false)
                        .open(&mut stats_open)
                        .show(ctx, |ui| {
                            ui.set_min_width(260.0);
                            let label_color = Color32::from_rgb(150, 200, 255);

                            egui::Grid::new("stats_grid")
                                .num_columns(2)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Total registros:").color(label_color));
                                    ui.label(format!("{}", stats.total));
                                    ui.end_row();
                                    ui.label(RichText::new("Valores no nulos:").color(label_color));
                                    ui.label(format!("{}", stats.non_null));
                                    ui.end_row();
                                    ui.label(RichText::new("Valores nulos:").color(label_color));
                                    ui.label(format!("{}", stats.total - stats.non_null));
                                    ui.end_row();
                                    ui.label(RichText::new("Valores unicos:").color(label_color));
                                    ui.label(format!("{}", stats.unique));
                                    ui.end_row();
                                });

                            if stats.is_numeric {
                                ui.separator();
                                egui::Grid::new("stats_num_grid")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        if let Some(v) = stats.min {
                                            ui.label(RichText::new("Minimo:").color(label_color));
                                            ui.label(format!("{v:.4}"));
                                            ui.end_row();
                                        }
                                        if let Some(v) = stats.max {
                                            ui.label(RichText::new("Maximo:").color(label_color));
                                            ui.label(format!("{v:.4}"));
                                            ui.end_row();
                                        }
                                        if let Some(v) = stats.mean {
                                            ui.label(RichText::new("Media:").color(label_color));
                                            ui.label(format!("{v:.4}"));
                                            ui.end_row();
                                        }
                                        if let Some(v) = stats.median {
                                            ui.label(RichText::new("Mediana:").color(label_color));
                                            ui.label(format!("{v:.4}"));
                                            ui.end_row();
                                        }
                                    });
                            }

                            if !stats.top_values.is_empty() {
                                ui.separator();
                                ui.label(
                                    RichText::new("Top 5 valores:")
                                        .color(label_color)
                                        .size(11.0),
                                );
                                let total = stats.total.max(1) as f64;
                                for (val, count) in &stats.top_values {
                                    let pct = *count as f64 / total * 100.0;
                                    let disp = if val.is_empty() { "null" } else { val.as_str() };
                                    ui.label(format!("  {disp}  ->  {count}  ({pct:.1}%)"));
                                }
                            }
                        });
                    if !stats_open {
                        self.tab_states[tab_idx].active_stats = None;
                    }
                }

                // ── Calculated column editor window ───────────────────────────
                let ts = &mut self.tab_states[tab_idx];
                if let Some((ref mut name_buf, ref mut expr_buf)) = ts.calc_col_editor {
                    let mut editor_open = true;
                    let mut apply = false;
                    let mut cancel = false;
                    let nb = name_buf.clone();
                    let eb = expr_buf.clone();

                    egui::Window::new("Nueva columna calculada")
                        .id(egui::Id::new("calc_col_editor").with(tab_idx))
                        .resizable(false)
                        .collapsible(false)
                        .open(&mut editor_open)
                        .show(ctx, |ui| {
                            ui.set_min_width(300.0);
                            let mut nb2 = nb.clone();
                            let mut eb2 = eb.clone();

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Nombre:")
                                        .color(Color32::from_rgb(150, 200, 255)),
                                );
                                ui.text_edit_singleline(&mut nb2);
                            });
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Expresion:")
                                        .color(Color32::from_rgb(150, 200, 255)),
                                );
                                ui.text_edit_singleline(&mut eb2);
                            });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Ej: precio * 1.19   upper(nombre)   len(campo)")
                                    .color(Color32::from_rgb(100, 130, 180))
                                    .size(10.0),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button("Aplicar").clicked() && !nb2.trim().is_empty() {
                                    apply = true;
                                }
                                if ui.button("Cancelar").clicked() {
                                    cancel = true;
                                }
                            });
                            // write back
                            if let Some((ref mut n, ref mut e)) = ts.calc_col_editor {
                                *n = nb2;
                                *e = eb2;
                            }
                        });

                    if apply {
                        if let Some((n, e)) = ts.calc_col_editor.take() {
                            let n = n.trim().to_string();
                            let e = e.trim().to_string();
                            if !n.is_empty() {
                                ts.calculated_cols.retain(|(cn, _)| cn != &n);
                                ts.calculated_cols.push((n, e));
                                ts.calc_col_dirty = true;
                            }
                        }
                    } else if cancel || !editor_open {
                        ts.calc_col_editor = None;
                    }
                }

                // ── Render floating filter panel window ──────────────────────
                let ds_records = self.datasets[tab_idx].records.clone();
                let ts = &mut self.tab_states[tab_idx];
                if let Some(panel) = ts.open_filter_panel.clone() {
                    let mut panel_open = true;
                    let fi_clone = ts.filtered_indices.clone();
                    if table::show_filter_panel(
                        ctx,
                        &panel,
                        &mut ts.filters,
                        &mut ts.easy_filters,
                        &mut ts.filter_mode,
                        &ds_records,
                        &fi_clone,
                        &mut panel_open,
                        tab_idx,
                    ) {
                        panel_filter_changed = true;
                    }
                    if !panel_open {
                        ts.open_filter_panel = None;
                    }
                }
            });

        if sort_changed || panel_filter_changed {
            self.recompute_filter();
        }
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    if dark {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = Color32::from_rgb(18, 24, 40);
        style.visuals.panel_fill = Color32::from_rgb(18, 24, 40);
        style.visuals.extreme_bg_color = Color32::from_rgb(12, 16, 28);
        style.visuals.code_bg_color = Color32::from_rgb(30, 40, 60);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 36, 60);
        style.visuals.widgets.inactive.fg_stroke =
            Stroke::new(1.0, Color32::from_rgb(140, 170, 220));
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 55, 90);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(50, 80, 150);
        style.visuals.selection.bg_fill = Color32::from_rgb(40, 80, 180);
        style.visuals.override_text_color = Some(Color32::from_rgb(210, 225, 245));
        ctx.set_style(style);
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
}

/// Extract a tab display name from a file path (file stem, no extension).
fn tab_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Dataset")
        .to_string()
}
