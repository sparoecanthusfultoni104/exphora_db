use crate::models::InferredSchema;
use egui::{Color32, RichText, ScrollArea, Ui};
use std::collections::HashMap;

/// Show the column visibility panel. Returns true if any column visibility changed.
pub fn show_columns_panel(
    ui: &mut Ui,
    schema: &InferredSchema,
    visible_columns: &mut HashMap<String, bool>,
) -> bool {
    let mut changed = false;

    ui.label(
        RichText::new("COLUMNAS")
            .size(11.0)
            .strong()
            .color(Color32::from_rgb(80, 100, 150)),
    );
    ui.separator();
    ui.add_space(4.0);

    // Show all / Hide all buttons
    ui.horizontal(|ui| {
        if ui
            .button(
                RichText::new("Mostrar todos")
                    .color(Color32::from_rgb(100, 220, 130))
                    .size(11.0),
            )
            .clicked()
        {
            for val in visible_columns.values_mut() {
                *val = true;
            }
            changed = true;
        }
        if ui
            .button(
                RichText::new("Ocultar todos")
                    .color(Color32::from_rgb(255, 150, 100))
                    .size(11.0),
            )
            .clicked()
        {
            for val in visible_columns.values_mut() {
                *val = false;
            }
            changed = true;
        }
    });

    ui.add_space(8.0);

    // Scrollable list of field checkboxes
    ScrollArea::vertical()
        .id_salt("columns_panel_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for field in &schema.fields {
                let vis = visible_columns.entry(field.name.clone()).or_insert(true);
                if ui.checkbox(vis, &field.name).changed() {
                    changed = true;
                }
            }
        });

    changed
}
