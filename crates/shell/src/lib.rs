//! Логика оболочки: триггеры у краёв экрана и Win32-примитивы.

pub mod trigger;
pub mod win32;

pub use trigger::{edge_at, ScreenRect, TriggerConfig};
pub use win32::{cursor_pos, primary_screen};
