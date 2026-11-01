//! UI-слой POS Desktop на Slint: панель приложений, статус-бар, сервисы.

use posdesktop_core::Edge;
use posdesktop_shell::{edge_at, win32, ScreenRect, TriggerConfig};
use std::time::{Duration, Instant};

slint::include_modules!();

const PANEL_WIDTH: i32 = 380;
const SLIDE_MS: u64 = 200;
const FRAME_MS: u64 = 16;
const SERVICES_HEIGHT: i32 = 48;
const CLOSE_DELAY_MS: u64 = 350;
const POLL_MS: u64 = 33;

pub fn run(overlay: bool) -> Result<(), slint::PlatformError> {
    let screen = win32::primary_screen();
    log::info!("экран {}x{}", screen.w, screen.h);

    let apps = AppWindow::new()?;
    apps.set_win_height(screen.h as f32);

    let status = StatusBar::new()?;
    status.set_bar_width(screen.w as f32);

    let services = ServicesBar::new()?;
    services.set_bar_width(screen.w as f32);

    apps.show()?;
    status.show()?;
    services.show()?;

    apps.window()
        .set_position(slint::PhysicalPosition::new(-PANEL_WIDTH, 0));
    status
        .window()
        .set_position(slint::PhysicalPosition::new(0, 0));
    services
        .window()
        .set_position(slint::PhysicalPosition::new(0, screen.h - SERVICES_HEIGHT));

    if overlay {
        for window in [apps.window(), status.window(), services.window()] {
            apply_overlay_style(window);
        }
    }

    spawn_trigger_worker(apps.as_weak(), screen);

    slint::run_event_loop()?;
    Ok(())
}

/// Снимает снапшот окна и печатает статистику цвета.
pub fn run_snapshot() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let weak = window.as_weak();

    slint::Timer::single_shot(Duration::from_millis(900), move || {
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

/// Проверка клик-сквозь: нижняя половина окна прозрачна для мыши.
pub fn run_clickthrough_test() -> Result<(), slint::PlatformError> {
    let screen = win32::primary_screen();
    let window = AppWindow::new()?;
    window.set_win_height(screen.h as f32);
    window.show()?;
    window
        .window()
        .set_position(slint::PhysicalPosition::new(0, 0));

    let weak = window.as_weak();
    let threshold = screen.h / 2;

    slint::Timer::single_shot(Duration::from_millis(400), move || {
        if let Some(component) = weak.upgrade() {
            if let Some(hwnd) = hwnd_of(component.window()) {
                install_clickthrough(hwnd, threshold);
                log::info!("clickthrough: hwnd=0x{hwnd:x}, pass-through ниже y={threshold}");
                println!("clickthrough hwnd=0x{hwnd:x} threshold_y={threshold}");
            } else {
                log::warn!("clickthrough: HWND не получен");
            }
        }
    });

    slint::Timer::single_shot(Duration::from_secs(8), || {
        let _ = slint::quit_event_loop();
    });

    slint::run_event_loop()?;
    Ok(())
}

fn spawn_trigger_worker(weak: slint::Weak<AppWindow>, screen: ScreenRect) {
    std::thread::spawn(move || {
        let config = TriggerConfig::default();
        let mut open = false;
        let mut leave_since: Option<Instant> = None;
        log::info!(
            "триггер запущен: зона {}px, углы {}px",
            config.zone_px,
            config.corner_px
        );

        loop {
            let Some((x, y)) = win32::cursor_pos() else {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            };

            if !open {
                if edge_at((x, y), screen, &config) == Some(Edge::Left) {
                    open = true;
                    leave_since = None;
                    log::info!("панель открывается, курсор ({x}, {y})");
                    animate(&weak, -PANEL_WIDTH, 0);
                }
            } else if x > PANEL_WIDTH + 24 {
                let now = Instant::now();
                let since = *leave_since.get_or_insert(now);
                if now.duration_since(since) >= Duration::from_millis(CLOSE_DELAY_MS) {
                    open = false;
                    leave_since = None;
                    log::info!("панель закрывается, курсор ({x}, {y})");
                    animate(&weak, 0, -PANEL_WIDTH);
                }
            } else {
                leave_since = None;
            }

            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    });
}

fn animate(weak: &slint::Weak<AppWindow>, from: i32, to: i32) {
    let steps = (SLIDE_MS / FRAME_MS).max(1) as i32;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from as f32 + (to - from) as f32 * t;
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                ui.window()
                    .set_position(slint::PhysicalPosition::new(x as i32, 0));
            }
        });
        std::thread::sleep(Duration::from_millis(FRAME_MS));
    }
}

#[cfg(windows)]
fn hwnd_of(window: &slint::Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle();
    let raw = handle.window_handle().ok()?;
    match raw.as_ref() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(windows))]
fn hwnd_of(_window: &slint::Window) -> Option<isize> {
    None
}

/// Превращает окно в оверлей: поверх всех, без кнопки на панели задач.
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
        log::info!("overlay: стили применены, hwnd=0x{hwnd:x}");
    }
}

#[cfg(not(windows))]
fn apply_overlay_style(_window: &slint::Window) {}

/// Подписывается на WM_NCHITTEST и отдаёт HTTRANSPARENT ниже порога.
#[cfg(windows)]
fn install_clickthrough(hwnd: isize, threshold_y: i32) {
    PASS_THROUGH_Y.store(threshold_y, std::sync::atomic::Ordering::Relaxed);
    unsafe {
        SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
}

#[cfg(windows)]
static PASS_THROUGH_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MAX);

#[cfg(windows)]
extern "system" fn subclass_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    const WM_NCHITTEST: u32 = 0x0084;
    const HTTRANSPARENT: isize = -1;

    if msg == WM_NCHITTEST {
        let y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
        if y > PASS_THROUGH_Y.load(std::sync::atomic::Ordering::Relaxed) {
            return HTTRANSPARENT;
        }
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

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

#[cfg(windows)]
#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(
        hwnd: isize,
        pfn_subclass: Option<extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize>,
        id: usize,
        data: usize,
    ) -> i32;
    fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
}
