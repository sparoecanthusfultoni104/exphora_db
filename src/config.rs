//! Centralised application paths.
//!
//! All code that needs to know *where* ExphoraDB stores user data should call
//! `app_data_dir()` rather than inlining `dirs::data_dir().join("ExphoraDB")`.

use std::path::PathBuf;

/// Returns the base directory for all ExphoraDB user data.
///
/// - Windows: `%APPDATA%\ExphoraDB`
/// - macOS:   `~/Library/Application Support/ExphoraDB`
/// - Linux:   `$XDG_DATA_HOME/ExphoraDB` (fallback: `~/.local/share/ExphoraDB`)
pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .expect("No se pudo obtener el directorio de datos del sistema")
        .join("ExphoraDB")
}
