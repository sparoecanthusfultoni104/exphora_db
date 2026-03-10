use crate::filters::DynamicFilters;
use crate::models::{Connector, FilterOp, FilterRule, InferredSchema};
use egui::{Color32, ComboBox, RichText, ScrollArea, Ui};
use std::collections::HashMap;

/// Render the sidebar with filters generated dynamically from the schema.
/// visible_columns controls which filter fields to show (hidden columns don't get filters).
/// Returns true if any filter changed.
pub fn show_sidebar(
    ui: &mut Ui,
    filters: &mut DynamicFilters,
    schema: &InferredSchema,
    visible_columns: &HashMap<String, bool>,
) -> bool {
    let mut changed = false;

    ScrollArea::vertical()
        .id_salt("sidebar_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // -- Text search --
            ui.add_space(4.0);
            ui.label(
                RichText::new("Busqueda")
                    .strong()
                    .color(Color32::from_rgb(150, 200, 255)),
            );
            ui.add_space(2.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut filters.text_search)
                    .hint_text("Buscar... | campo: \"valor\"")
                    .desired_width(f32::INFINITY),
            );
            if resp.changed() {
                changed = true;
            }
            ui.add_space(2.0);
            ui.label(
                RichText::new("Ej: apellido1: \"Garcia\" grado: \"9\"")
                    .size(10.0)
                    .color(Color32::from_rgb(90, 120, 170)),
            );

            // -- Dropdown filters (only for visible columns) --
            let visible_filter_fields: Vec<&String> = schema
                .filter_fields
                .iter()
                .filter(|f| *visible_columns.get(f.as_str()).unwrap_or(&true))
                .collect();

            if !visible_filter_fields.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Filtros")
                        .strong()
                        .color(Color32::from_rgb(150, 200, 255)),
                );
                ui.add_space(4.0);

                for field_name in &visible_filter_fields {
                    if let Some(meta) = schema.fields.iter().find(|f| &&f.name == field_name) {
                        // Get current equals value from first rule if any
                        let current = filters
                            .filters
                            .get(field_name.as_str())
                            .and_then(|rules| rules.first())
                            .and_then(|r| {
                                if r.op == FilterOp::Equals {
                                    Some(r.value.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();

                        let mut selected = current;
                        let did_change =
                            dropdown(ui, &meta.name, &mut selected, &meta.unique_values);

                        if did_change {
                            if selected.is_empty() {
                                filters.filters.remove(field_name.as_str());
                            } else {
                                filters.filters.insert(
                                    field_name.to_string(),
                                    vec![FilterRule {
                                        op: FilterOp::Equals,
                                        value: selected,
                                        connector: Connector::And,
                                    }],
                                );
                            }
                            changed = true;
                        }
                    }
                }
            }

            // -- Boolean checkboxes (only for visible columns) --
            let visible_bool_fields: Vec<&String> = schema
                .bool_fields
                .iter()
                .filter(|f| *visible_columns.get(f.as_str()).unwrap_or(&true))
                .collect();

            if !visible_bool_fields.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Opciones")
                        .strong()
                        .color(Color32::from_rgb(150, 200, 255)),
                );
                ui.add_space(4.0);

                for field_name in &visible_bool_fields {
                    let is_active = filters
                        .filters
                        .get(field_name.as_str())
                        .and_then(|rules| rules.first())
                        .map(|r| r.op == FilterOp::BoolTrue)
                        .unwrap_or(false);
                    let mut checked = is_active;
                    if ui
                        .checkbox(&mut checked, format!("Solo {field_name}: true"))
                        .changed()
                    {
                        if checked {
                            filters.filters.insert(
                                field_name.to_string(),
                                vec![FilterRule {
                                    op: FilterOp::BoolTrue,
                                    value: String::new(),
                                    connector: Connector::And,
                                }],
                            );
                        } else {
                            filters.filters.remove(field_name.as_str());
                        }
                        changed = true;
                    }
                }
            }

            // -- Reset button --
            ui.add_space(12.0);
            if ui
                .button(RichText::new("Limpiar filtros").color(Color32::from_rgb(255, 150, 100)))
                .clicked()
            {
                filters.reset();
                changed = true;
            }
        });

    changed
}

fn dropdown(ui: &mut Ui, label: &str, selected: &mut String, options: &[String]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(format!("{label}:"));
        ComboBox::from_id_salt(label)
            .selected_text(if selected.is_empty() {
                "Todos"
            } else {
                selected.as_str()
            })
            .width(130.0)
            .show_ui(ui, |ui| {
                if ui.selectable_label(selected.is_empty(), "Todos").clicked() {
                    selected.clear();
                    changed = true;
                }
                for opt in options {
                    if ui.selectable_label(selected == opt, opt.as_str()).clicked() {
                        *selected = opt.clone();
                        changed = true;
                    }
                }
            });
    });
    changed
}
