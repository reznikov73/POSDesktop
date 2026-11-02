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

/// Рабочая область основного монитора — экран без панели задач.
#[cfg(windows)]
pub fn primary_work_area() -> ScreenRect {
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, data: *mut Rect, win_ini: u32) -> i32;
    }

    const SPI_GETWORKAREA: u32 = 0x0030;

    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect, 0) };
    if ok == 0 {
        log::warn!("SPI_GETWORKAREA не сработал, беру весь экран");
        return primary_screen();
    }
    ScreenRect {
        x: rect.left,
        y: rect.top,
        w: rect.right - rect.left,
        h: rect.bottom - rect.top,
    }
}

#[cfg(not(windows))]
pub fn primary_work_area() -> ScreenRect {
    primary_screen()
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
