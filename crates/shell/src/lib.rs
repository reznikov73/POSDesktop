//! Логика оболочки: триггеры у краёв экрана и Win32-примитивы.

pub mod tray;
pub mod trigger;
pub mod win32;

pub use tray::{TrayCommand, TrayHandle};
pub use trigger::{edge_at, ScreenRect, TriggerConfig};
pub use win32::{cursor_pos, primary_screen, primary_work_area};
