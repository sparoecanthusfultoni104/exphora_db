use crate::models::{val_to_str, InferredSchema, JsonRecord};
use egui::{Color32, ComboBox, RichText, ScrollArea, Ui};
use std::collections::HashMap;

/// Show the stats panel for the active dataset.
pub fn show_stats_panel(
    ui: &mut Ui,
    records: &[JsonRecord],
    filtered_indices: &[usize],
    schema: &InferredSchema,
    selected_field: &mut String,
) {
    ui.add_space(4.0);
    ui.label(
        RichText::new("Estadisticas")
            .strong()
            .size(14.0)
            .color(Color32::from_rgb(150, 220, 255)),
    );
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Record counts
    let total = records.len();
    let filtered = filtered_indices.len();
    ui.label(
        RichText::new(format!("Registros filtrados: {filtered} / {total}"))
            .color(Color32::from_rgb(180, 210, 240))
            .size(12.0),
    );
    ui.add_space(8.0);

    // Top 5 fields with most nulls
    ui.label(
        RichText::new("Campos con mas nulos")
            .strong()
            .color(Color32::from_rgb(150, 200, 255))
            .size(12.0),
    );
    ui.add_space(2.0);
    let mut null_fields: Vec<_> = schema
        .fields
        .iter()
        .filter(|f| f.non_null_ratio < 1.0)
        .map(|f| (&f.name, ((1.0 - f.non_null_ratio) * total as f64) as usize))
        .collect();
    null_fields.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, null_count) in null_fields.iter().take(5) {
        ui.label(
            RichText::new(format!("  {name}: {null_count} nulos"))
                .color(Color32::from_rgb(200, 200, 220))
                .size(11.0),
        );
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(4.0);

    // Field selector for frequency analysis
    ui.label(
        RichText::new("Frecuencia por campo")
            .strong()
            .color(Color32::from_rgb(150, 200, 255))
            .size(12.0),
    );
    ui.add_space(4.0);

    let field_names: Vec<String> = schema.fields.iter().map(|f| f.name.clone()).collect();
    if selected_field.is_empty() && !field_names.is_empty() {
        *selected_field = field_names[0].clone();
    }

    ComboBox::from_id_salt("stats_field_combo")
        .selected_text(if selected_field.is_empty() {
            "Seleccionar campo"
        } else {
            selected_field.as_str()
        })
        .width(160.0)
        .show_ui(ui, |ui| {
            for name in &field_names {
                if ui
                    .selectable_label(selected_field == name, name.as_str())
                    .clicked()
                {
                    *selected_field = name.clone();
                }
            }
        });

    ui.add_space(8.0);

    // Compute top 10 values for the selected field on filtered set
    if !selected_field.is_empty() {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for &idx in filtered_indices {
            if let Some(obj) = records.get(idx).and_then(|r| r.as_object()) {
                let val = obj
                    .get(selected_field.as_str())
                    .map(|v| val_to_str(v))
                    .unwrap_or_else(|| "(vacio)".to_string());
                let val = if val.is_empty() {
                    "(vacio)".to_string()
                } else {
                    val
                };
                *freq.entry(val).or_insert(0) += 1;
            }
        }

        let mut sorted: Vec<_> = freq.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        ui.label(
            RichText::new(format!("Top 10 valores de \"{}\":", selected_field))
                .color(Color32::from_rgb(150, 190, 255))
                .size(12.0),
        );
        ui.add_space(4.0);

        ScrollArea::vertical()
            .id_salt("stats_freq_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                for (val, count) in sorted.iter().take(10) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{count}"))
                                .color(Color32::from_rgb(255, 200, 100))
                                .size(12.0)
                                .strong(),
                        );
                        // Truncate long values for display
                        let display_val = if val.len() > 40 {
                            format!("{}...", &val[..37])
                        } else {
                            val.clone()
                        };
                        ui.label(
                            RichText::new(display_val)
                                .color(Color32::from_rgb(200, 210, 230))
                                .size(12.0),
                        );
                    });
                }
            });
    }
}
