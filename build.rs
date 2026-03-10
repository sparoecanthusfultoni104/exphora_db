use chrono::Local;

fn main() {
    tauri_build::build(); // Required: embeds Windows Application Manifest for Common Controls v6

    let now = Local::now();
    let formatted = now.format("%Y-%m-%d %H:%M").to_string();
    println!("cargo:rustc-env=BUILD_DATE={formatted}");
}
