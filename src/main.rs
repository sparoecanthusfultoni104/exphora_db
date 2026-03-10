mod commands;
pub mod config;
mod expr;
mod filters;
mod models;
mod p2p;
mod parser;

use commands::p2p_ops::P2pState;
use tokio::sync::mpsc;

const BUILD_DATE: &str = env!("BUILD_DATE");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // ── P2P async runtime (background OS thread) ─────────────────────────────
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (cmd_tx, cmd_rx) = mpsc::channel::<p2p::Command>(32);
    std::thread::spawn(move || rt.block_on(p2p::run_event_loop(cmd_rx)));

    let p2p_state = P2pState { cmd_tx };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            let win = app.get_webview_window("main").unwrap();
            let title = format!("Exphora DB --Dev {}", BUILD_DATE);
            win.set_title(&title).unwrap();

            #[cfg(debug_assertions)]
            win.open_devtools();

            #[cfg(target_os = "windows")]
            {
                let win = app.get_webview_window("main").unwrap();
                win.set_title("ExphoraDB").unwrap();
            }

            Ok(())
        })
        .manage(p2p_state)
        .manage(commands::AppInfoState {
            version: VERSION.to_string(),
            build_date: BUILD_DATE.to_string(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::open_file_dialog,
            commands::load_file,
            commands::apply_filters,
            commands::get_unique_values,
            commands::get_column_stats,
            commands::eval_calc_column,
            commands::export_format,
            commands::p2p_share,
            commands::p2p_fetch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
