//! UI-слой POS Desktop на Slint.

slint::include_modules!();

/// Открывает окно приложения и запускает цикл событий.
///
/// `overlay = true` — окно становится оверлеем: поверх всех, без кнопки
/// на панели задач, не забирает фокус. Без флага — обычное окно.
pub fn run(overlay: bool) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    log::info!("окно создано");

    if overlay {
        let weak = window.as_weak();
        // HWND появляется только после создания окна бэкендом.
        slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
            if let Some(component) = weak.upgrade() {
                apply_overlay_style(component.window());
            }
        });
    }

    window.run()
}

/// Открывает окно, снимает снапшот и завершает работу.
/// Используется для проверки темы без визуального осмотра.
pub fn run_snapshot() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let weak = window.as_weak();

    slint::Timer::single_shot(std::time::Duration::from_millis(700), move || {
        if let Some(component) = weak.upgrade() {
            match component.window().take_snapshot() {
                Ok(buffer) => {
                    let (width, height) = (buffer.width(), buffer.height());
                    let (mut red, mut green, mut blue) = (0u64, 0u64, 0u64);
                    let mut dark = 0u64;
                    let mut content = 0u64;
                    for pixel in buffer.as_slice() {
                        red += pixel.r as u64;
                        green += pixel.g as u64;
                        blue += pixel.b as u64;
                        if pixel.r < 100 && pixel.g < 100 && pixel.b < 100 {
                            dark += 1;
                        }
                        // Пиксели, отличные от фона #fbfcfc.
                        if pixel.r.abs_diff(251) > 4
                            || pixel.g.abs_diff(252) > 4
                            || pixel.b.abs_diff(252) > 4
                        {
                            content += 1;
                        }
                    }
                    let count = (width as u64 * height as u64).max(1);
                    println!(
                        "snapshot {width}x{height} avg=({},{},{}) dark_pixels={dark} content_pixels={content} ({:.1}%)",
                        red / count,
                        green / count,
                        blue / count,
                        content as f64 / count as f64 * 100.0
                    );
                }
                Err(err) => eprintln!("snapshot error: {err}"),
            }
        }
        let _ = slint::quit_event_loop();
    });

    window.run()
}

/// Превращает окно в оверлей.
#[cfg(windows)]
fn apply_overlay_style(window: &slint::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let slint_handle = window.window_handle();
    let Ok(handle) = slint_handle.window_handle() else {
        log::warn!("overlay: окно ещё не создано, стили не применены");
        return;
    };

    let RawWindowHandle::Win32(win32) = handle.as_ref() else {
        return;
    };
    let hwnd = win32.hwnd.get();

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // Убираем APPWINDOW (кнопка на панели задач), ставим TOOLWINDOW и NOACTIVATE.
        let new_style = (ex_style & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        log::info!("overlay: TOPMOST|TOOLWINDOW|NOACTIVATE применены, hwnd=0x{hwnd:x}");
    }
}

#[cfg(not(windows))]
fn apply_overlay_style(_window: &slint::Window) {}

#[cfg(windows)]
const GWL_EXSTYLE: i32 = -20;
#[cfg(windows)]
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
#[cfg(windows)]
const WS_EX_APPWINDOW: isize = 0x0004_0000;
#[cfg(windows)]
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
#[cfg(windows)]
const HWND_TOPMOST: isize = -1;
#[cfg(windows)]
const SWP_NOSIZE: u32 = 0x0001;
#[cfg(windows)]
const SWP_NOMOVE: u32 = 0x0002;
#[cfg(windows)]
const SWP_NOACTIVATE: u32 = 0x0010;

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn SetWindowPos(
        hwnd: isize,
        insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}
