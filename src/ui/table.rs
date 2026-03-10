use crate::models::{
    val_to_str, EasyFilter, FilterMode, FilterOp, FilterPanel, FilterRule, InferredSchema,
    JsonRecord,
};
use egui::{Color32, RichText, Sense, Ui};
use std::collections::HashMap;

pub enum TableAction {
    None,
    ClickRow(usize),
    ClickHeader(String),
    ShowColStats(String),
    ToggleFreeze(String),
    RemoveCalcCol(String),
}

#[allow(dead_code)]
const MIN_COL_W: f32 = 80.0;
const DEFAULT_COL_W: f32 = 140.0;
const MAX_CHARS_CELL: usize = 22;
const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 30.0;
const FROZEN_SEPARATOR_W: f32 = 3.0;

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        format!("{}...", s.chars().take(max_chars).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Look up a cell value — checks calc_col_cache first, then the record object.
fn get_cell_value(
    col: &str,
    record_idx: usize,
    obj: Option<&serde_json::Map<String, serde_json::Value>>,
    calc_col_cache: &HashMap<String, Vec<Option<String>>>,
) -> String {
    if let Some(vals) = calc_col_cache.get(col) {
        return vals
            .get(record_idx)
            .and_then(|v| v.as_deref())
            .unwrap_or("")
            .to_string();
    }
    obj.and_then(|o| o.get(col))
        .map(|v| val_to_str(v))
        .unwrap_or_default()
}

/// Returns the display width for `col` from `col_widths`, or `DEFAULT_COL_W`.
fn col_w(col: &str, col_widths: &HashMap<String, f32>) -> f32 {
    *col_widths.get(col).unwrap_or(&DEFAULT_COL_W)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main table
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn show_table(
    ui: &mut Ui,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    // vis_cols already includes calculated columns appended after schema cols
    visible_cols: &[String],
    frozen_cols: &[String],
    calc_col_names: &[String], // subset of visible_cols that are calculated
    calc_col_cache: &HashMap<String, Vec<Option<String>>>,
    col_widths: &HashMap<String, f32>,
    sort_col: Option<&str>,
    sort_asc: bool,
    selected_row: Option<usize>,
    search_text: &str,
    schema: &InferredSchema,
    open_filter_panel: &mut Option<FilterPanel>,
    active_filters: &HashMap<String, Vec<FilterRule>>,
    tab_index: usize,
) -> TableAction {
    let mut action = TableAction::None;

    if visible_cols.is_empty() {
        ui.label(RichText::new("Sin columnas disponibles").color(Color32::from_rgb(120, 140, 180)));
        return action;
    }

    // Split visible cols into frozen and unfrozen (preserving order within each group)
    let frozen_set: std::collections::HashSet<&str> =
        frozen_cols.iter().map(|s| s.as_str()).collect();

    // Frozen cols: use frozen_cols order but only include those actually in visible_cols
    let vis_set: std::collections::HashSet<&str> =
        visible_cols.iter().map(|s| s.as_str()).collect();
    let frozen: Vec<&String> = frozen_cols
        .iter()
        .filter(|c| vis_set.contains(c.as_str()))
        .collect();
    let unfrozen: Vec<&String> = visible_cols
        .iter()
        .filter(|c| !frozen_set.contains(c.as_str()))
        .collect();

    let frozen_w: f32 = frozen.iter().map(|c| col_w(c, col_widths)).sum::<f32>();
    let unfrozen_w: f32 = unfrozen.iter().map(|c| col_w(c, col_widths)).sum::<f32>();

    let num_rows = filtered_indices.len();
    let total_height = HEADER_HEIGHT + num_rows as f32 * ROW_HEIGHT;

    // Colors
    let hdr_bg = Color32::from_rgb(30, 40, 60);
    let hdr_color = Color32::from_rgb(150, 200, 255);
    let sort_color = Color32::from_rgb(255, 200, 100);
    let filter_color = Color32::from_rgb(80, 200, 80);
    let filter_bg = Color32::from_rgba_unmultiplied(40, 120, 40, 180);
    let row_bg = Color32::from_rgb(22, 28, 42);
    let row_alt_bg = Color32::from_rgb(28, 36, 52);
    let row_sel_bg = Color32::from_rgba_unmultiplied(100, 150, 255, 40);
    let highlight_color = Color32::from_rgb(255, 220, 80);
    let frozen_sep_color = Color32::from_rgb(80, 120, 200);
    let calc_col_color = Color32::from_rgb(180, 255, 180); // green tint for calc col headers

    let is_free_search = !search_text.is_empty() && !search_text.contains(':');
    let search_lower = search_text.to_lowercase();

    let calc_set: std::collections::HashSet<&str> =
        calc_col_names.iter().map(|s| s.as_str()).collect();

    // ── Outer: vertical scroll only ───────────────────────────────────────────
    egui::ScrollArea::vertical()
        .id_salt(egui::Id::new("main_table_vscroll").with(tab_index))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(total_height);

            // ── Row layout: frozen block | separator | horizontal scroll ───────
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                // ── LEFT: Frozen columns (no horizontal scroll) ───────────────
                if !frozen.is_empty() {
                    ui.vertical(|ui| {
                        ui.set_width(frozen_w);
                        ui.set_min_height(total_height);

                        // Headers
                        ui.horizontal(|ui| {
                            for col_name in &frozen {
                                let w = col_w(col_name, col_widths);
                                render_header_cell(
                                    ui,
                                    col_name,
                                    w,
                                    sort_col,
                                    sort_asc,
                                    active_filters,
                                    frozen_cols,
                                    calc_set.contains(col_name.as_str()),
                                    schema,
                                    open_filter_panel,
                                    hdr_bg,
                                    hdr_color,
                                    sort_color,
                                    filter_color,
                                    filter_bg,
                                    calc_col_color,
                                    &mut action,
                                );
                            }
                        });

                        ui.separator();

                        // Rows
                        render_rows(
                            ui,
                            records,
                            filtered_indices,
                            &frozen,
                            calc_col_cache,
                            col_widths,
                            selected_row,
                            is_free_search,
                            &search_lower,
                            highlight_color,
                            row_bg,
                            row_alt_bg,
                            row_sel_bg,
                            frozen_w,
                            &mut action,
                        );
                    });

                    // Vertical separator between frozen and unfrozen
                    let sep_rect = {
                        let r = ui.available_rect_before_wrap();
                        egui::Rect::from_min_size(
                            r.min,
                            egui::vec2(FROZEN_SEPARATOR_W, total_height),
                        )
                    };
                    ui.painter().rect_filled(sep_rect, 0.0, frozen_sep_color);
                    ui.add_space(FROZEN_SEPARATOR_W);
                }

                // ── RIGHT: Unfrozen columns (horizontal scroll) ───────────────
                egui::ScrollArea::horizontal()
                    .id_salt(egui::Id::new("main_table_hscroll").with(tab_index))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(unfrozen_w);
                        ui.set_min_height(total_height);
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                        ui.vertical(|ui| {
                            // Headers
                            ui.horizontal(|ui| {
                                for col_name in &unfrozen {
                                    let w = col_w(col_name, col_widths);
                                    render_header_cell(
                                        ui,
                                        col_name,
                                        w,
                                        sort_col,
                                        sort_asc,
                                        active_filters,
                                        frozen_cols,
                                        calc_set.contains(col_name.as_str()),
                                        schema,
                                        open_filter_panel,
                                        hdr_bg,
                                        hdr_color,
                                        sort_color,
                                        filter_color,
                                        filter_bg,
                                        calc_col_color,
                                        &mut action,
                                    );
                                }
                            });

                            ui.separator();

                            // Rows
                            render_rows(
                                ui,
                                records,
                                filtered_indices,
                                &unfrozen,
                                calc_col_cache,
                                col_widths,
                                selected_row,
                                is_free_search,
                                &search_lower,
                                highlight_color,
                                row_bg,
                                row_alt_bg,
                                row_sel_bg,
                                unfrozen_w,
                                &mut action,
                            );
                        });
                    });
            });
        });

    action
}

// ─────────────────────────────────────────────────────────────────────────────
// Header cell renderer
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_header_cell(
    ui: &mut Ui,
    col_name: &str,
    w: f32,
    sort_col: Option<&str>,
    sort_asc: bool,
    active_filters: &HashMap<String, Vec<FilterRule>>,
    frozen_cols: &[String],
    is_calc_col: bool,
    schema: &InferredSchema,
    open_filter_panel: &mut Option<FilterPanel>,
    hdr_bg: Color32,
    hdr_color: Color32,
    sort_color: Color32,
    filter_color: Color32,
    filter_bg: Color32,
    calc_col_color: Color32,
    action: &mut TableAction,
) {
    let is_sorted = sort_col == Some(col_name);
    let has_filter = active_filters
        .get(col_name)
        .map(|rules| !rules.is_empty())
        .unwrap_or(false);
    let is_frozen = frozen_cols.contains(&col_name.to_string());

    let arrow = if is_sorted {
        if sort_asc {
            " ^"
        } else {
            " v"
        }
    } else {
        ""
    };

    let label_text = if is_calc_col {
        format!("[{col_name}]{arrow}")
    } else {
        format!("{col_name}{arrow}")
    };

    let text_color = if has_filter {
        filter_color
    } else if is_calc_col {
        calc_col_color
    } else if is_sorted {
        sort_color
    } else {
        hdr_color
    };

    let btn_fill = if has_filter { filter_bg } else { hdr_bg };

    let btn = ui.add_sized(
        [w, HEADER_HEIGHT],
        egui::Button::new(RichText::new(&label_text).strong().color(text_color))
            .fill(btn_fill)
            .frame(true),
    );

    // Left click → sort
    if btn.clicked() {
        *action = TableAction::ClickHeader(col_name.to_string());
    }

    // Right click → context menu
    let col_owned = col_name.to_string();
    let frozen_count = frozen_cols.len();
    btn.context_menu(|ui| {
        ui.label(
            RichText::new(format!("Columna: {col_owned}"))
                .color(Color32::from_rgb(150, 180, 230))
                .size(11.0),
        );
        ui.separator();

        // Ver estadisticas
        if ui.button("Ver estadisticas").clicked() {
            *action = TableAction::ShowColStats(col_owned.clone());
            ui.close_menu();
        }

        // Filtro de columna — moved from direct right-click to menu
        if ui.button("Filtro de columna").clicked() {
            let pos = btn.rect.left_bottom() + egui::vec2(0.0, 2.0);
            let field_meta = schema.fields.iter().find(|f| f.name == col_owned);
            if let Some(meta) = field_meta {
                *open_filter_panel = Some(FilterPanel {
                    column: col_owned.clone(),
                    field_type: meta.field_type.clone(),
                    unique_values: meta.unique_values.clone(),
                    position: pos,
                });
            }
            ui.close_menu();
        }

        ui.separator();

        // Freeze / unfreeze
        if is_frozen {
            if ui.button("Desfijar columna").clicked() {
                *action = TableAction::ToggleFreeze(col_owned.clone());
                ui.close_menu();
            }
        } else if frozen_count >= 5 {
            ui.add_enabled(false, egui::Button::new("Fijar columna"))
                .on_disabled_hover_text("Maximo 5 columnas fijadas");
        } else if ui.button("Fijar columna").clicked() {
            *action = TableAction::ToggleFreeze(col_owned.clone());
            ui.close_menu();
        }

        // Remove calculated column
        if is_calc_col {
            ui.separator();
            if ui.button("Quitar columna calculada").clicked() {
                *action = TableAction::RemoveCalcCol(col_owned.clone());
                ui.close_menu();
            }
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Row block renderer (shared between frozen and unfrozen sides)
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_rows(
    ui: &mut Ui,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    cols: &[&String],
    calc_col_cache: &HashMap<String, Vec<Option<String>>>,
    col_widths: &HashMap<String, f32>,
    selected_row: Option<usize>,
    is_free_search: bool,
    search_lower: &str,
    highlight_color: Color32,
    row_bg: Color32,
    row_alt_bg: Color32,
    row_sel_bg: Color32,
    block_width: f32,
    action: &mut TableAction,
) {
    let num_rows = filtered_indices.len();

    // Virtualization
    let clip_top = ui.clip_rect().min.y;
    let content_top = ui.min_rect().min.y;
    let scroll_y = (clip_top - content_top).max(0.0);
    let visible_height = ui.clip_rect().height().max(400.0);
    let first_row = (scroll_y / ROW_HEIGHT).floor() as usize;
    let visible_rows = (visible_height / ROW_HEIGHT).ceil() as usize + 2;
    let visible_rows = visible_rows.max(30);
    let last_row = (first_row + visible_rows).min(num_rows);

    if first_row > 0 {
        ui.add_space(first_row as f32 * ROW_HEIGHT);
    }

    for row_i in first_row..last_row {
        let Some(&record_idx) = filtered_indices.get(row_i) else {
            continue;
        };
        let Some(record) = records.get(record_idx) else {
            continue;
        };
        let obj = record.as_object();
        let is_selected = selected_row == Some(row_i);

        let bg = if row_i % 2 == 0 { row_bg } else { row_alt_bg };

        // Row background rect
        let row_rect = {
            let r = ui.available_rect_before_wrap();
            egui::Rect::from_min_size(r.min, egui::vec2(block_width.max(r.width()), ROW_HEIGHT))
        };
        ui.painter().rect_filled(row_rect, 0.0, bg);
        if is_selected {
            ui.painter().rect_filled(row_rect, 0.0, row_sel_bg);
        }

        // Cells
        let row_resp = ui.horizontal(|ui| {
            ui.set_min_width(block_width);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            for col_name in cols {
                let w = col_w(col_name, col_widths);
                let full_text = get_cell_value(col_name, record_idx, obj, calc_col_cache);
                let display = truncate_str(&full_text, MAX_CHARS_CELL);

                let highlight = is_free_search && full_text.to_lowercase().contains(search_lower);
                let label_text = if highlight {
                    RichText::new(display).color(highlight_color).strong()
                } else {
                    RichText::new(display)
                };

                let cell_resp = ui
                    .add_sized([w, ROW_HEIGHT], egui::Label::new(label_text))
                    .on_hover_text(&full_text);

                // Right-click context on cell
                let col_owned = col_name.to_string();
                let visible_cols_snap: Vec<String> = cols.iter().map(|c| c.to_string()).collect();
                let record_clone = record.clone();
                cell_resp.context_menu(|ui| {
                    ui.label(
                        RichText::new(format!("Campo: {col_owned}"))
                            .color(Color32::from_rgb(150, 180, 230))
                            .size(11.0),
                    );
                    ui.separator();
                    if ui.button("Copiar fila").clicked() {
                        let obj_inner = record_clone.as_object();
                        let text = visible_cols_snap
                            .iter()
                            .map(|col| {
                                let v = obj_inner
                                    .and_then(|o| o.get(col))
                                    .map(|v| val_to_str(v))
                                    .unwrap_or_default();
                                format!("{col}: {v}")
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        ui.ctx().copy_text(text);
                        ui.close_menu();
                    }
                    if ui.button("Copiar como CSV").clicked() {
                        let obj_inner = record_clone.as_object();
                        let text = visible_cols_snap
                            .iter()
                            .map(|col| {
                                obj_inner
                                    .and_then(|o| o.get(col))
                                    .map(|v| val_to_str(v))
                                    .unwrap_or_default()
                            })
                            .collect::<Vec<_>>()
                            .join(",");
                        ui.ctx().copy_text(text);
                        ui.close_menu();
                    }
                });
            }
        });

        if row_resp.response.interact(Sense::click()).clicked() {
            *action = TableAction::ClickRow(row_i);
        }
    }

    // Bottom spacer
    let remaining_rows = num_rows.saturating_sub(last_row);
    if remaining_rows > 0 {
        ui.add_space(remaining_rows as f32 * ROW_HEIGHT);
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Filter panel window
// ───────────────────────────────────────────────────────────────────────────────

/// Compute (value_string, count) pairs for `column` from `records[filtered_indices]`.
/// Null/empty values are represented as the sentinel string "null".
/// Returns at most `max_unique` entries, sorted by descending frequency.
fn compute_unique_values(
    column: &str,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    max_unique: usize,
) -> Vec<(String, usize)> {
    let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for &idx in filtered_indices {
        let cell = records
            .get(idx)
            .and_then(|r| r.as_object())
            .and_then(|o| o.get(column))
            .map(|v| val_to_str(v))
            .unwrap_or_default();
        let key = if cell.is_empty() {
            "null".to_string()
        } else {
            cell
        };
        *freq.entry(key).or_insert(0) += 1;
    }
    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    // Sort by freq desc, then alphabetically for stability
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    pairs.truncate(max_unique);
    pairs
}

/// Render the dual-mode per-column filter panel (Easy checkboxes + Advanced rules).
/// Returns true if any filter changed (caller should recompute).
#[allow(clippy::too_many_arguments)]
pub fn show_filter_panel(
    ctx: &egui::Context,
    panel: &FilterPanel,
    // Advanced-mode rules
    adv_filters: &mut HashMap<String, Vec<FilterRule>>,
    // Easy-mode filter state
    easy_filters: &mut HashMap<String, EasyFilter>,
    // Per-column mode selection
    filter_mode: &mut HashMap<String, FilterMode>,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    open: &mut bool,
    tab_index: usize,
) -> bool {
    let mut filter_changed = false;
    let col = panel.column.clone();

    egui::Window::new(format!("Filtro: \"{}\"", col))
        .id(egui::Id::new("col_filter_panel").with(tab_index))
        .fixed_pos(panel.position)
        .resizable(false)
        .collapsible(false)
        .min_width(300.0)
        .open(open)
        .show(ctx, |ui| {
            // ── Mode toggle bar ─────────────────────────────────────────────
            let mode = filter_mode.entry(col.clone()).or_default().clone();
            let sel_fill = ui.visuals().selection.bg_fill;
            let default_fill = ui.visuals().widgets.inactive.bg_fill;

            ui.horizontal(|ui| {
                let easy_active = mode == FilterMode::Easy;
                let adv_active = mode == FilterMode::Advanced;

                if ui
                    .add(
                        egui::Button::new(RichText::new("Facil").size(12.0)).fill(if easy_active {
                            sel_fill
                        } else {
                            default_fill
                        }),
                    )
                    .clicked()
                    && !easy_active
                {
                    *filter_mode.entry(col.clone()).or_default() = FilterMode::Easy;
                    filter_changed = true;
                }

                if ui
                    .add(
                        egui::Button::new(RichText::new("Avanzado").size(12.0))
                            .fill(if adv_active { sel_fill } else { default_fill }),
                    )
                    .clicked()
                    && !adv_active
                {
                    *filter_mode.entry(col.clone()).or_default() = FilterMode::Advanced;
                    filter_changed = true;
                }
            });
            ui.separator();

            let mode = filter_mode.entry(col.clone()).or_default().clone();

            match mode {
                // ── FACIL MODE – checkboxes ─────────────────────────────────
                FilterMode::Easy => {
                    const MAX_UNIQUE: usize = 200;

                    let all_pairs =
                        compute_unique_values(&col, records, filtered_indices, MAX_UNIQUE);
                    let showed_top =
                        all_pairs.len() == MAX_UNIQUE && filtered_indices.len() > MAX_UNIQUE;

                    // Initialise EasyFilter on first open (all selected)
                    let ef = easy_filters.entry(col.clone()).or_insert_with(|| {
                        EasyFilter::all_checked(all_pairs.iter().map(|(v, _)| v.clone()))
                    });

                    // Local search (visual only)
                    let search_id = egui::Id::new("easy_filter_search")
                        .with(tab_index)
                        .with(&col);
                    let mut local_search: String =
                        ctx.data(|d| d.get_temp::<String>(search_id).unwrap_or_default());

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Buscar:")
                                .size(11.0)
                                .color(Color32::from_rgb(120, 160, 210)),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut local_search)
                                .desired_width(180.0)
                                .hint_text("buscar valor..."),
                        );
                    });
                    ctx.data_mut(|d| d.insert_temp(search_id, local_search.clone()));
                    let search_lower = local_search.to_lowercase();

                    if showed_top {
                        ui.label(
                            RichText::new(format!("Mostrando top {MAX_UNIQUE} valores"))
                                .size(10.0)
                                .color(Color32::from_rgb(200, 160, 80)),
                        );
                    }

                    ui.add_space(4.0);

                    // Checkbox list
                    egui::ScrollArea::vertical()
                        .id_salt(
                            egui::Id::new("easy_filter_scroll")
                                .with(tab_index)
                                .with(&col),
                        )
                        .max_height(220.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let displayed: Vec<&(String, usize)> = all_pairs
                                .iter()
                                .filter(|(v, _)| {
                                    search_lower.is_empty()
                                        || v.to_lowercase().contains(&search_lower)
                                })
                                .collect();

                            for (val, count) in &displayed {
                                let is_checked =
                                    ef.all_selected || ef.selected.contains(val.as_str());
                                let mut checked = is_checked;
                                if ui
                                    .checkbox(&mut checked, format!("{val}  ({count})"))
                                    .changed()
                                {
                                    if ef.all_selected {
                                        // First uncheck: populate selected with all, then remove
                                        ef.selected =
                                            all_pairs.iter().map(|(v, _)| v.clone()).collect();
                                        ef.all_selected = false;
                                    }
                                    if checked {
                                        ef.selected.insert(val.to_string());
                                    } else {
                                        ef.selected.remove(val.as_str());
                                    }
                                    if ef.selected.len() == all_pairs.len() {
                                        ef.all_selected = true;
                                    }
                                    filter_changed = true;
                                }
                            }
                        });

                    ui.add_space(4.0);
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Seleccionar todo").clicked() {
                            let ef2 = easy_filters.entry(col.clone()).or_default();
                            ef2.selected = all_pairs.iter().map(|(v, _)| v.clone()).collect();
                            ef2.all_selected = true;
                            filter_changed = true;
                        }
                        if ui
                            .button(
                                RichText::new("Limpiar").color(Color32::from_rgb(220, 120, 100)),
                            )
                            .clicked()
                        {
                            // Limpiar marca all_selected = true (no limpia el HashSet).
                            // Eso basta para que apply_easy_filter devuelva true para todo.
                            // El HashSet queda poblado para que el proximo panel abra rapido.
                            let ef2 = easy_filters.entry(col.clone()).or_default();
                            ef2.selected = all_pairs.iter().map(|(v, _)| v.clone()).collect();
                            ef2.all_selected = true;
                            filter_changed = true;
                        }
                    });
                }

                // ── ADVANCED MODE – multi-rule editor ───────────────────────
                FilterMode::Advanced => {
                    let rules = adv_filters.entry(col.clone()).or_default();

                    if rules.is_empty() {
                        rules.push(FilterRule::default());
                    }

                    let n = rules.len();
                    let mut to_remove: Option<usize> = None;
                    let mut add_rule = false;
                    let mut clear = false;

                    for i in 0..rules.len() {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(
                                egui::Id::new("adv_filter_op").with(tab_index).with(i),
                            )
                            .selected_text(rules[i].op.label())
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                for op in FilterOp::all_text() {
                                    let selected = &rules[i].op == op;
                                    if ui.selectable_label(selected, op.label()).clicked()
                                        && rules[i].op != *op
                                    {
                                        rules[i].op = op.clone();
                                        filter_changed = true;
                                    }
                                }
                            });

                            if rules[i].op.needs_value() {
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut rules[i].value)
                                            .desired_width(100.0)
                                            .hint_text("valor"),
                                    )
                                    .changed()
                                {
                                    filter_changed = true;
                                }
                            }

                            if n > 1
                                && ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("x")
                                                .color(Color32::from_rgb(220, 80, 80)),
                                        )
                                        .small(),
                                    )
                                    .clicked()
                            {
                                to_remove = Some(i);
                                filter_changed = true;
                            }
                        });

                        if i + 1 < rules.len() {
                            ui.horizontal(|ui| {
                                let conn_label = rules[i].connector.label();
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(conn_label)
                                                .color(Color32::from_rgb(200, 200, 100))
                                                .size(11.0),
                                        )
                                        .small()
                                        .fill(Color32::from_rgb(40, 40, 20)),
                                    )
                                    .clicked()
                                {
                                    rules[i].connector.toggle();
                                    filter_changed = true;
                                }
                            });
                        }
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("+").clicked() {
                            add_rule = true;
                        }
                        if ui
                            .button(
                                RichText::new("Limpiar").color(Color32::from_rgb(220, 120, 100)),
                            )
                            .clicked()
                        {
                            clear = true;
                        }
                    });

                    // Mutations after loop to avoid double-borrow
                    let rules = adv_filters.entry(col.clone()).or_default();
                    if let Some(idx) = to_remove {
                        rules.remove(idx);
                        filter_changed = true;
                    }
                    if add_rule {
                        rules.push(FilterRule::default());
                    }
                    if clear {
                        rules.clear();
                        filter_changed = true;
                    }
                }
            }
        });

    filter_changed
}
