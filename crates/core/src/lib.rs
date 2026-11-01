//! Ядро POS Desktop: модель раскладки, хранилище, логирование.

pub mod layout;
pub mod logging;
pub mod store;

pub use layout::{
    Density, Edge, Group, Layout, LayoutError, Object, ObjectKind, Panel, PanelMode, PanelType,
    Rect, Screen, GRID_STEP, LAYOUT_VERSION, TILE_UNIT,
};
pub use store::{default_path as default_layout_path, load_or_default, save};

/// Версия крейта из Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
