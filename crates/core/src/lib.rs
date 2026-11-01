//! Ядро POS Desktop: модель раскладки, валидация, логирование.

pub mod layout;
pub mod logging;

pub use layout::{
    Density, Edge, Group, Layout, LayoutError, Object, ObjectKind, Panel, PanelMode, PanelType,
    Rect, Screen, GRID_STEP, LAYOUT_VERSION, TILE_UNIT,
};

/// Версия крейта из Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
