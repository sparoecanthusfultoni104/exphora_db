use egui::{Color32, RichText, ScrollArea, Ui};

pub fn show_detail(ui: &mut Ui, title: &str, fields: &[(String, String)]) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(title)
            .strong()
            .size(15.0)
            .color(Color32::from_rgb(150, 220, 255)),
    );
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    ScrollArea::vertical()
        .id_salt("detail_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (key, val) in fields {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("{key}:"))
                            .strong()
                            .color(Color32::from_rgb(150, 190, 255))
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new(val)
                            .color(Color32::from_rgb(220, 230, 240))
                            .size(12.0),
                    );
                });
                ui.add_space(2.0);
            }
        });
}

pub fn show_no_selection(ui: &mut Ui) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Selecciona un registro\npara ver sus detalles")
                .color(Color32::from_rgb(100, 120, 160))
                .size(13.0),
        );
    });
}
