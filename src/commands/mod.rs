pub mod calc_ops;
pub mod export_ops;
pub mod file_ops;
pub mod filter_ops;
pub mod p2p_ops;
pub mod view_ops;

pub use calc_ops::*;
pub use export_ops::*;
pub use file_ops::*;
pub use filter_ops::*;
pub use p2p_ops::*;
pub use view_ops::*;

// ── AppInfo state (version + build_date) ────────────────────────────────────

pub struct AppInfoState {
    pub version: String,
    pub build_date: String,
}
