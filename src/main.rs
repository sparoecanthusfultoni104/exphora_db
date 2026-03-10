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

#[cfg(target_os = "windows")]
fn register_file_association() {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_ALL_ACCESS};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = match hkcu.open_subkey_with_flags("Software\\Classes", KEY_ALL_ACCESS) {
        Ok(k) => k,
        Err(_) => return, // Si no podemos abrir HKCU\Software\Classes, salimos silenciosamente
    };

    // Obtenemos la ruta del ejecutable actual
    let exe_path = match std::env::current_exe() {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(_) => return,
    };

    // 1. .exh -> "ExphoraDB.View"
    if let Ok((ext_key, _)) = classes.create_subkey(".exh") {
        let _ = ext_key.set_value("", &"ExphoraDB.View");
    }

    // 2. ExphoraDB.View -> "ExphoraDB View"
    if let Ok((prog_key, _)) = classes.create_subkey("ExphoraDB.View") {
        let _ = prog_key.set_value("", &"ExphoraDB View");

        // 3. DefaultIcon
        if let Ok((icon_key, _)) = prog_key.create_subkey("DefaultIcon") {
            let icon_str = format!("{},0", exe_path);
            let _ = icon_key.set_value("", &icon_str);
        }

        // 4. shell\open\command
        if let Ok((shell_key, _)) = prog_key.create_subkey("shell\\open\\command") {
            let cmd_str = format!("\"{}\" \"%1\"", exe_path);
            let _ = shell_key.set_value("", &cmd_str);
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    register_file_association();

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
            commands::save_file,
            commands::apply_filters,
            commands::get_unique_values,
            commands::get_column_stats,
            commands::eval_calc_column,
            commands::export_format,
            commands::p2p_share,
            commands::p2p_fetch,
            commands::save_view,
            commands::load_view,
            commands::relink_view,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
