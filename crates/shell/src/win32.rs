//! Win32: позиция курсора и размер основного монитора.

use crate::trigger::ScreenRect;

#[cfg(windows)]
mod ffi {
    #[repr(C)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetCursorPos(point: *mut Point) -> i32;
        pub fn GetSystemMetrics(index: i32) -> i32;
        pub fn GetAsyncKeyState(vk: i32) -> i16;
    }

    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;
    pub const VK_LBUTTON: i32 = 0x01;
}

/// Позиция курсора в физических пикселях.
#[cfg(windows)]
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = ffi::Point { x: 0, y: 0 };
    let ok = unsafe { ffi::GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x, point.y))
}

#[cfg(not(windows))]
pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}

/// Нажата ли левая кнопка мыши.
#[cfg(windows)]
pub fn left_button_down() -> bool {
    (unsafe { ffi::GetAsyncKeyState(ffi::VK_LBUTTON) } as u16) & 0x8000 != 0
}

#[cfg(not(windows))]
pub fn left_button_down() -> bool {
    false
}

/// Прямоугольник основного монитора.
#[cfg(windows)]
pub fn primary_screen() -> ScreenRect {
    ScreenRect {
        x: 0,
        y: 0,
        w: unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) },
        h: unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) },
    }
}

#[cfg(not(windows))]
pub fn primary_screen() -> ScreenRect {
    ScreenRect {
        x: 0,
        y: 0,
        w: 1280,
        h: 800,
    }
}
