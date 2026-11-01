//! UI-слой POS Desktop на Slint: панель приложений, статус-бар, сервисы.

use posdesktop_core::{Edge, Layout};
use posdesktop_shell::{edge_at, win32, ScreenRect, TriggerConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod drag;

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

    let layout_path = posdesktop_core::default_layout_path();
    let existed = layout_path.exists();
    let layout = posdesktop_core::load_or_default(&layout_path);
    if !existed {
        match posdesktop_core::save(&layout_path, &layout) {
            Ok(()) => log::info!("layout.json создан: {}", layout_path.display()),
            Err(err) => log::warn!("не удалось сохранить layout.json: {err}"),
        }
    }
    let workspace = WorkspaceWindow::new()?;
    // Окно покрывает только габариты плиток: полноэкранная прозрачная
    // поверхность стоит лишних 15–20 МБ памяти.
    let workspace_state = drag::install(&workspace, layout, layout_path);
    let (width, height) = workspace_state.borrow().window_size();
    workspace.set_ws_width(width as f32);
    workspace.set_ws_height(height as f32);

    workspace.show()?;
    workspace
        .window()
        .set_position(slint::PhysicalPosition::new(0, 0));

    // HWND появляется только после первого прохода цикла событий.
    let workspace_weak = workspace.as_weak();
    let state_for_style = workspace_state.clone();
    slint::Timer::single_shot(Duration::from_millis(350), move || {
        if let Some(component) = workspace_weak.upgrade() {
            if let Some(hwnd) = hwnd_of(component.window()) {
                apply_tool_style(hwnd);
                let rects = state_for_style
                    .borrow()
                    .physical_rects(component.window().scale_factor());
                install_clickthrough_rects(hwnd, &rects);
                let rect = window_rect(hwnd);
                log::info!(
                    "workspace: клик-сквозь вне плиток, hwnd=0x{hwnd:x}, rect=({}, {}) {}x{}",
                    rect.0,
                    rect.1,
                    rect.2,
                    rect.3
                );
            } else {
                log::warn!("workspace: HWND не получен");
            }
        }
    });

    if overlay {
        for window in [apps.window(), status.window(), services.window()] {
            apply_overlay_style(window);
        }
    }

    let panel_state = Arc::new(PanelState::new());
    spawn_trigger_worker(apps.as_weak(), screen, panel_state.clone());
    spawn_hotkey_worker(apps.as_weak(), panel_state);

    slint::run_event_loop()?;
    log::info!("цикл событий завершён");
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

fn tiles_model(layout: &Layout) -> Vec<Tile> {
    layout
        .monitors
        .values()
        .flat_map(|screen| screen.objects.iter())
        .map(drag::tile_of)
        .collect()
}

/// Снапшот рабочей области: проверяет прозрачность фона.
pub fn run_workspace_snapshot() -> Result<(), slint::PlatformError> {
    let screen = win32::primary_screen();
    let layout = posdesktop_core::load_or_default(&posdesktop_core::default_layout_path());

    let window = WorkspaceWindow::new()?;
    window.set_ws_width(screen.w as f32);
    window.set_ws_height(screen.h as f32);
    window.set_tiles(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(tiles_model(&layout)),
    )));

    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(900), move || {
        if let Some(component) = weak.upgrade() {
            if let Ok(buffer) = component.window().take_snapshot() {
                let (width, height) = (buffer.width(), buffer.height());
                let mut transparent = 0u64;
                let mut opaque = 0u64;
                let mut alpha_sum = 0u64;
                for pixel in buffer.as_slice() {
                    alpha_sum += pixel.a as u64;
                    if pixel.a == 0 {
                        transparent += 1;
                    }
                    if pixel.a == 255 {
                        opaque += 1;
                    }
                }
                let count = (width as u64 * height as u64).max(1);
                println!(
                    "workspace {width}x{height} transparent_px={transparent} ({:.1}%) opaque_px={opaque} avg_alpha={}",
                    transparent as f64 / count as f64 * 100.0,
                    alpha_sum / count
                );
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
                let rects = vec![(0, 0, threshold * 2, threshold)];
                install_clickthrough_rects(hwnd, &rects);
                log::info!("clickthrough: hwnd=0x{hwnd:x}, интерактивен верх до y={threshold}");
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

/// Состояние панели, общее для триггера и хоткея.
struct PanelState {
    open: AtomicBool,
    /// Панель открыта с клавиатуры — триггер её не закрывает.
    sticky: AtomicBool,
}

impl PanelState {
    fn new() -> Self {
        Self {
            open: AtomicBool::new(false),
            sticky: AtomicBool::new(false),
        }
    }
}

fn open_panel(weak: &slint::Weak<AppWindow>, state: &PanelState, reason: &str, sticky: bool) {
    if state.open.swap(true, Ordering::SeqCst) {
        return;
    }
    state.sticky.store(sticky, Ordering::SeqCst);
    log::info!("панель открывается ({reason})");
    animate(weak, -PANEL_WIDTH, 0);
}

fn close_panel(weak: &slint::Weak<AppWindow>, state: &PanelState, reason: &str) {
    if !state.open.swap(false, Ordering::SeqCst) {
        return;
    }
    state.sticky.store(false, Ordering::SeqCst);
    log::info!("панель закрывается ({reason})");
    animate(weak, 0, -PANEL_WIDTH);
}

fn spawn_trigger_worker(weak: slint::Weak<AppWindow>, screen: ScreenRect, state: Arc<PanelState>) {
    std::thread::spawn(move || {
        let config = TriggerConfig::default();
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

            if !state.open.load(Ordering::SeqCst) {
                if edge_at((x, y), screen, &config) == Some(Edge::Left) {
                    leave_since = None;
                    open_panel(
                        &weak,
                        &state,
                        &format!("край экрана, курсор ({x}, {y})"),
                        false,
                    );
                }
            } else if !state.sticky.load(Ordering::SeqCst) && x > PANEL_WIDTH + 24 {
                let now = Instant::now();
                let since = *leave_since.get_or_insert(now);
                if now.duration_since(since) >= Duration::from_millis(CLOSE_DELAY_MS) {
                    leave_since = None;
                    close_panel(&weak, &state, &format!("курсор ({x}, {y})"));
                }
            } else {
                leave_since = None;
            }

            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    });
}

/// Глобальный хоткей Ctrl+Alt+1 — открыть/закрыть панель приложений.
#[cfg(windows)]
fn spawn_hotkey_worker(weak: slint::Weak<AppWindow>, state: Arc<PanelState>) {
    std::thread::spawn(move || {
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        const MOD_SHIFT: u32 = 0x0004;
        const MOD_NOREPEAT: u32 = 0x4000;
        const VK_1: u32 = 0x31;

        let base = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
        let mut label = "Ctrl+Alt+1";
        let mut registered = unsafe { RegisterHotKey(0, HOTKEY_PANEL, base, VK_1) };

        if registered == 0 {
            let code = unsafe { GetLastError() };
            log::warn!("Ctrl+Alt+1 занят (код {code}), пробую Ctrl+Alt+Shift+1");
            registered = unsafe { RegisterHotKey(0, HOTKEY_PANEL, base | MOD_SHIFT, VK_1) };
            if registered != 0 {
                label = "Ctrl+Alt+Shift+1";
            } else {
                let code = unsafe { GetLastError() };
                log::warn!("Ctrl+Alt+Shift+1 тоже занят (код {code}); хоткей отключён");
                return;
            }
        }

        log::info!("хоткей {label} зарегистрирован");

        loop {
            let mut buffer = [0u8; 64];
            let result =
                unsafe { GetMessageW(buffer.as_mut_ptr() as *mut std::ffi::c_void, 0, 0, 0) };
            if result <= 0 {
                break;
            }
            if state.open.load(Ordering::SeqCst) {
                close_panel(&weak, &state, &format!("хоткей {label}"));
            } else {
                open_panel(&weak, &state, &format!("хоткей {label}"), true);
            }
        }
    });
}

#[cfg(not(windows))]
fn spawn_hotkey_worker(_weak: slint::Weak<AppWindow>, _state: Arc<PanelState>) {}

const HOTKEY_PANEL: i32 = 1;

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

/// Подписывается на WM_NCHITTEST: интерактивны только указанные прямоугольники.
#[cfg(windows)]
fn install_clickthrough_rects(hwnd: isize, rects: &[(i32, i32, i32, i32)]) {
    update_pass_rects(rects);
    CLICKTHROUGH.store(true, Ordering::Relaxed);
    unsafe {
        SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
}

/// Обновляет интерактивные зоны без повторного сабклассинга (после перетаскивания).
#[cfg(windows)]
pub(crate) fn update_pass_rects(rects: &[(i32, i32, i32, i32)]) {
    if let Ok(mut guard) = PASS_RECTS.lock() {
        guard.clear();
        guard.extend_from_slice(rects);
    }
}

#[cfg(not(windows))]
pub(crate) fn update_pass_rects(_rects: &[(i32, i32, i32, i32)]) {}

#[cfg(windows)]
static PASS_RECTS: Mutex<Vec<(i32, i32, i32, i32)>> = Mutex::new(Vec::new());
#[cfg(windows)]
static CLICKTHROUGH: AtomicBool = AtomicBool::new(false);

/// Окно без кнопки на панели задач, не забирает фокус, без topmost.
#[cfg(windows)]
fn apply_tool_style(hwnd: isize) {
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (ex_style & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
    }
}

/// Прямоугольник окна в экранных координатах.
#[cfg(windows)]
fn window_rect(hwnd: isize) -> (i32, i32, i32, i32) {
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    (
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    )
}

#[cfg(not(windows))]
fn window_rect(_hwnd: isize) -> (i32, i32, i32, i32) {
    (0, 0, 0, 0)
}

/// Опускает окно в самый низ z-порядка (не используется).
#[cfg(windows)]
#[allow(dead_code)]
fn send_to_bottom(hwnd: isize) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

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
    const HTCLIENT: isize = 1;

    if msg == WM_NCHITTEST && CLICKTHROUGH.load(Ordering::Relaxed) {
        let x = (lparam & 0xFFFF) as u16 as i16 as i32;
        let y = ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32;
        let inside = PASS_RECTS
            .lock()
            .map(|rects| {
                rects
                    .iter()
                    .any(|&(rx, ry, rw, rh)| x >= rx && x < rx + rw && y >= ry && y < ry + rh)
            })
            .unwrap_or(false);
        return if inside { HTCLIENT } else { HTTRANSPARENT };
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
const HWND_BOTTOM: isize = 1;
#[cfg(windows)]
const SWP_NOSIZE: u32 = 0x0001;
#[cfg(windows)]
const SWP_NOMOVE: u32 = 0x0002;
#[cfg(windows)]
const SWP_NOACTIVATE: u32 = 0x0010;

#[cfg(windows)]
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, vk: u32) -> i32;
    fn GetMessageW(
        msg: *mut std::ffi::c_void,
        hwnd: isize,
        filter_min: u32,
        filter_max: u32,
    ) -> i32;
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
#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
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
