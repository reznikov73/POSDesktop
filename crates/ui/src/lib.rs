//! UI-слой POS Desktop на Slint: панель приложений, статус-бар, сервисы.

use posdesktop_core::{Edge, Layout};
use posdesktop_shell::{edge_at, win32, ScreenRect, TriggerConfig};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod action;
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
    // Панели док-режима живут в рабочей области: иначе нижняя уезжает под панель задач.
    let work = win32::primary_work_area();
    log::info!(
        "экран {}x{}, рабочая область {}x{} в ({}, {})",
        screen.w,
        screen.h,
        work.w,
        work.h,
        work.x,
        work.y
    );

    // Окно папки: открывается поверх рабочей области рядом с плиткой.
    let folder = FolderWindow::new()?;
    folder.hide()?;

    let apps = AppWindow::new()?;
    apps.set_win_height(work.h as f32);

    let status = StatusBar::new()?;
    status.set_bar_width(work.w as f32);
    status.set_mail_text(mail_text().into());

    let services = ServicesBar::new()?;
    services.set_bar_width(work.w as f32);

    apps.show()?;
    status.show()?;
    services.show()?;

    // Панели — часть оболочки: кнопки на панели задач им не нужны.
    let panels = (apps.as_weak(), status.as_weak(), services.as_weak());

    apps.window()
        .set_position(slint::PhysicalPosition::new(work.x - PANEL_WIDTH, work.y));
    status
        .window()
        .set_position(slint::PhysicalPosition::new(work.x, work.y));
    services.window().set_position(slint::PhysicalPosition::new(
        work.x,
        work.y + work.h - SERVICES_HEIGHT,
    ));

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
        hide_from_taskbar(&panels.0);
        hide_from_taskbar(&panels.1);
        hide_from_taskbar(&panels.2);
        #[cfg(windows)]
        {
            // Второй экземпляр просит показать оболочку широковещательным сообщением.
            if let Some(app) = panels.0.upgrade() {
                if let Some(hwnd) = hwnd_of(app.window()) {
                    install_show_message(hwnd);
                }
            }
        }
        if let Some(component) = workspace_weak.upgrade() {
            if let Some(hwnd) = hwnd_of(component.window()) {
                apply_tool_style(hwnd);
                accept_dropped_files(hwnd);
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
    // Трей и второй экземпляр управляют панелью через общий обработчик.
    let _tray = install_actions(
        &apps,
        &status,
        &services,
        workspace_state.clone(),
        &workspace,
        &folder,
        panel_state.clone(),
    );
    spawn_trigger_worker(apps.as_weak(), screen, panel_state.clone());
    spawn_hotkey_worker();

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

/// Снапшот рабочей области в BMP: видны плитки и их иконки.
pub fn run_tiles_snapshot(path: &std::path::Path) -> Result<(), slint::PlatformError> {
    let layout = posdesktop_core::load_or_default(&posdesktop_core::default_layout_path());
    let tiles = tiles_model(&layout);
    let mut width = 320i32;
    let mut height = 240i32;
    for tile in &tiles {
        width = width.max(tile.x as i32 + tile.w as i32 + 16);
        height = height.max(tile.y as i32 + tile.h as i32 + 16);
    }

    let window = WorkspaceWindow::new()?;
    window.set_ws_width(width as f32);
    window.set_ws_height(height as f32);
    window.set_tiles(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(tiles),
    )));

    let weak = window.as_weak();
    let path = path.to_path_buf();
    slint::Timer::single_shot(Duration::from_millis(900), move || {
        if let Some(component) = weak.upgrade() {
            if let Ok(buffer) = component.window().take_snapshot() {
                match write_bmp(&path, buffer.width(), buffer.height(), buffer.as_slice()) {
                    Ok(()) => println!(
                        "tiles snapshot: {} ({}x{})",
                        path.display(),
                        buffer.width(),
                        buffer.height()
                    ),
                    Err(err) => eprintln!("не удалось записать {}: {err}", path.display()),
                }
            }
        }
        let _ = slint::quit_event_loop();
    });

    window.run()
}

/// Снапшот панели приложений в BMP: проверка вёрстки без окон поверх.
pub fn run_panel_snapshot(path: &std::path::Path) -> Result<(), slint::PlatformError> {
    let work = win32::primary_work_area();
    let window = AppWindow::new()?;
    window.set_win_height(work.h as f32);

    let weak = window.as_weak();
    let path = path.to_path_buf();
    slint::Timer::single_shot(Duration::from_millis(900), move || {
        if let Some(component) = weak.upgrade() {
            match component.window().take_snapshot() {
                Ok(buffer) => {
                    let pixels = buffer.as_slice();
                    let clear = pixels.iter().filter(|pixel| pixel.a == 0).count();
                    println!(
                        "panel alpha: прозрачных {clear} из {} ({:.1}%)",
                        pixels.len(),
                        clear as f64 / pixels.len().max(1) as f64 * 100.0
                    );
                    let result = write_bmp(&path, buffer.width(), buffer.height(), pixels);
                    match result {
                        Ok(()) => println!(
                            "panel snapshot: {} ({}x{})",
                            path.display(),
                            buffer.width(),
                            buffer.height()
                        ),
                        Err(err) => eprintln!("не удалось записать {}: {err}", path.display()),
                    }
                }
                Err(err) => eprintln!("snapshot error: {err}"),
            }
        }
        let _ = slint::quit_event_loop();
    });

    window.run()
}

/// Пишет BMP 24 бит: свой формат без внешних крейтов, читается любым просмотрщиком.
fn write_bmp(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixels: &[slint::Rgba8Pixel],
) -> std::io::Result<()> {
    let row = (width * 3 + 3) & !3;
    let image_size = row * height;
    let mut out = Vec::with_capacity(54 + image_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(54 + image_size).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&image_size.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..height).rev() {
        let start = (y * width) as usize;
        let mut line = Vec::with_capacity(row as usize);
        for pixel in &pixels[start..start + width as usize] {
            line.push(pixel.b);
            line.push(pixel.g);
            line.push(pixel.r);
        }
        line.resize(row as usize, 0);
        out.extend_from_slice(&line);
    }
    std::fs::write(path, out)
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

/// Подключает действия из трея и от второго экземпляра.
///
/// Возвращённый дескриптор держит иконку трея живой; при выходе из `run` иконка
/// удаляется вместе с ним.
fn install_actions(
    apps: &AppWindow,
    status: &StatusBar,
    services: &ServicesBar,
    workspace: Rc<RefCell<drag::Workspace>>,
    workspace_ui: &WorkspaceWindow,
    folder: &FolderWindow,
    state: Arc<PanelState>,
) -> Option<posdesktop_shell::TrayHandle> {
    let weak = apps.as_weak();
    let workspace_weak = workspace_ui.as_weak();
    let folder_weak = folder.as_weak();
    let open_folder = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));

    {
        let folder_for_click = folder.as_weak();
        let apps_for_click = open_folder.clone();
        folder.on_item_clicked(move |index| {
            let Ok(index) = usize::try_from(index) else {
                return;
            };
            let app = apps_for_click.borrow().get(index).cloned();
            if let Some(app) = app {
                drag::launch_app(&app);
            }
            if let Some(window) = folder_for_click.upgrade() {
                let _ = window.hide();
            }
        });
    }

    {
        let folder_for_close = folder.as_weak();
        folder.on_close_requested(move || {
            if let Some(window) = folder_for_close.upgrade() {
                let _ = window.hide();
            }
        });
    }
    let status_weak = status.as_weak();
    let services_weak = services.as_weak();
    let status_visible = std::cell::Cell::new(true);
    let services_visible = std::cell::Cell::new(true);

    action::set_handler(move |command| match command {
        action::Action::ShowPanel => {
            open_panel(&weak, &state, "запрос от второго экземпляра", true)
        }
        action::Action::TogglePanels => {
            if state.open.load(Ordering::SeqCst) {
                close_panel(&weak, &state, "трей");
            } else {
                open_panel(&weak, &state, "трей", true);
            }
        }
        action::Action::ToggleStatus => toggle_dock(&status_weak, &status_visible, "статус"),
        action::Action::ToggleServices => toggle_dock(&services_weak, &services_visible, "сервисы"),
        action::Action::HideAll => {
            close_panel(&weak, &state, "свернуть все");
            hide_dock(&status_weak, &status_visible);
            hide_dock(&services_weak, &services_visible);
            log::info!("все панели свёрнуты");
        }
        action::Action::Drop { x, y, paths } => {
            if let Some(ui) = workspace_weak.upgrade() {
                let scale = ui.window().scale_factor().max(0.5);
                workspace
                    .borrow_mut()
                    .drop_items(&ui, x as f32 / scale, y as f32 / scale, paths);
            }
        }
        action::Action::OpenFolder(index) => {
            let Some(window) = folder_weak.upgrade() else {
                return;
            };
            let Some((title, items, rect)) = workspace.borrow().folder(index) else {
                return;
            };
            *open_folder.borrow_mut() = workspace.borrow().folder_apps(index);

            let rows = items.len().div_ceil(4).max(1) as f32;
            let height = 28.0 + 22.0 + 12.0 + rows * 92.0 + (rows - 1.0) * 10.0;
            window.set_content_height(height);
            window.set_folder_title(title.clone().into());
            window.set_items(slint::ModelRc::from(std::rc::Rc::new(
                slint::VecModel::from(items),
            )));

            let work = posdesktop_shell::win32::primary_work_area();
            let scale = window.window().scale_factor().max(0.5);
            let width = (348.0 * scale) as i32;
            let mut x = rect.right() + 8;
            if x + width / scale as i32 > work.x + work.w {
                x = (rect.x - 8 - width / scale as i32).max(work.x);
            }
            let y = rect.y.min(work.y + work.h - height as i32).max(work.y);
            let _ = window.show();
            window
                .window()
                .set_position(slint::PhysicalPosition::new(x, y));
            log::info!("папка «{title}»: приложений {}", open_folder.borrow().len());
        }
        action::Action::Undo => {
            if let Some(ui) = workspace_weak.upgrade() {
                workspace.borrow_mut().undo(&ui);
            }
        }
        action::Action::Redo => {
            if let Some(ui) = workspace_weak.upgrade() {
                workspace.borrow_mut().redo(&ui);
            }
        }
        action::Action::OpenLogs => open_logs_folder(),
        action::Action::Quit => {
            log::info!("выход по команде из трея");
            let _ = slint::quit_event_loop();
        }
    });

    let tray = posdesktop_shell::tray::spawn(|command| {
        use posdesktop_shell::TrayCommand;
        let action = match command {
            TrayCommand::TogglePanels => action::Action::TogglePanels,
            TrayCommand::Undo => action::Action::Undo,
            TrayCommand::Redo => action::Action::Redo,
            TrayCommand::OpenLogs => action::Action::OpenLogs,
            TrayCommand::Quit => action::Action::Quit,
        };
        action::post(action);
    });
    if tray.is_none() {
        log::warn!("иконка в трее не создана");
    }
    tray
}

/// Показывает или скрывает док-панель по горячей клавише.
fn toggle_dock<T: slint::ComponentHandle + 'static>(
    weak: &slint::Weak<T>,
    visible: &std::cell::Cell<bool>,
    name: &str,
) {
    let Some(window) = weak.upgrade() else {
        return;
    };
    if visible.get() {
        if let Err(err) = window.hide() {
            log::warn!("не удалось скрыть панель {name}: {err}");
        }
        visible.set(false);
        log::info!("панель {name} скрыта");
    } else if let Err(err) = window.show() {
        log::warn!("не удалось показать панель {name}: {err}");
    } else {
        visible.set(true);
        log::info!("панель {name} показана");
    }
}

/// Скрывает док-панель без переключения.
fn hide_dock<T: slint::ComponentHandle + 'static>(
    weak: &slint::Weak<T>,
    visible: &std::cell::Cell<bool>,
) {
    if let Some(window) = weak.upgrade() {
        let _ = window.hide();
        visible.set(false);
    }
}

/// Запуск приложения по названию из каталога «Пуск» (CLI и будущий инструмент ИИ).
pub fn run_launch(name: &str) -> bool {
    let Some(path) = posdesktop_shell::icons::find_shortcut(name) else {
        log::warn!("запуск {name}: ярлык в меню «Пуск» не найден");
        println!("ярлык не найден: {name}");
        return false;
    };
    if posdesktop_shell::launch::open(&path) {
        println!("запуск {name}: {}", path.display());
        true
    } else {
        println!("ShellExecute не открыл {}", path.display());
        false
    }
}

/// Счётчик почты из Soroka; если базы нет — прочерк.
fn mail_text() -> String {
    let Some(mail) = posdesktop_integrations::SorokaMail::open() else {
        log::warn!("почта: Soroka не найдена, показываю прочерк");
        return "Почта —".to_string();
    };
    match mail.read() {
        Some(summary) => {
            log::info!(
                "почта Soroka: {} непрочитанных, всего {}",
                summary.unread_total(),
                summary.total
            );
            format!("Почта {}", summary.unread_total())
        }
        None => {
            log::warn!("почта: счётчики Soroka не прочитаны, показываю прочерк");
            "Почта —".to_string()
        }
    }
}

/// Открывает каталог логов в Проводнике.
fn open_logs_folder() {
    let dir = posdesktop_core::logging::log_dir();
    log::info!("открываю логи: {}", dir.display());
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
}

/// Подписывается на сообщение «показать оболочку» от второго экземпляра.
#[cfg(windows)]
fn install_show_message(hwnd: isize) {
    unsafe {
        SetWindowSubclass(hwnd, Some(apps_subclass_proc), 2, 0);
    }
}

#[cfg(windows)]
extern "system" fn apps_subclass_proc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    if msg != 0 && msg == show_message() {
        action::dispatch(action::Action::ShowPanel);
        return 0;
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Идентификатор сообщения «показать оболочку», общий для всех экземпляров.
#[cfg(windows)]
fn show_message() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    static ID: AtomicU32 = AtomicU32::new(0);
    let cached = ID.load(AtomicOrdering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let name: Vec<u16> = "POSDesktop.Show\0".encode_utf16().collect();
    let id = unsafe { RegisterWindowMessageW(name.as_ptr()) };
    ID.store(id, AtomicOrdering::Relaxed);
    id
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

/// Глобальные хоткеи панелей (ТЗ §5.6).
///
/// Ctrl+Alt+1 — приложения, Ctrl+Alt+3 — статус, Ctrl+Alt+4 — сервисы,
/// Ctrl+Alt+Esc — свернуть все. Если комбинация занята, пробуется вариант с Shift.
/// Ctrl+Alt+2 закреплён за панелью ИИ и появится вместе с ней.
#[cfg(windows)]
fn spawn_hotkey_worker() {
    std::thread::spawn(move || {
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        const MOD_SHIFT: u32 = 0x0004;
        const MOD_NOREPEAT: u32 = 0x4000;
        const WM_HOTKEY: u32 = 0x0312;

        let base = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
        let specs: [(i32, u32, &str, action::Action); 6] = [
            (1, 0x31, "Ctrl+Alt+1", action::Action::TogglePanels),
            (3, 0x33, "Ctrl+Alt+3", action::Action::ToggleStatus),
            (4, 0x34, "Ctrl+Alt+4", action::Action::ToggleServices),
            (5, 0x1B, "Ctrl+Alt+Esc", action::Action::HideAll),
            (6, 0x5A, "Ctrl+Alt+Z", action::Action::Undo),
            (7, 0x59, "Ctrl+Alt+Y", action::Action::Redo),
        ];

        let mut registered: Vec<(i32, action::Action)> = Vec::new();
        for (id, key, label, command) in specs {
            if unsafe { RegisterHotKey(0, id, base, key) } != 0 {
                log::info!("хоткей {label} зарегистрирован");
                registered.push((id, command));
                continue;
            }
            let code = unsafe { GetLastError() };
            if unsafe { RegisterHotKey(0, id, base | MOD_SHIFT, key) } != 0 {
                log::info!("хоткей {label} занят (код {code}), работает Shift-вариант");
                registered.push((id, command));
            } else {
                log::warn!("хоткей {label} недоступен (код {code})");
            }
        }
        log::info!("Ctrl+Alt+2 закреплён за панелью ИИ — она ещё не реализована");

        if registered.is_empty() {
            return;
        }

        loop {
            let mut message = Msg::default();
            let result =
                unsafe { GetMessageW(&mut message as *mut Msg as *mut std::ffi::c_void, 0, 0, 0) };
            if result <= 0 {
                break;
            }
            if message.message != WM_HOTKEY {
                continue;
            }
            let id = message.w_param as i32;
            if let Some((_, command)) = registered.iter().find(|(hotkey, _)| *hotkey == id) {
                action::post(command.clone());
            }
        }
    });
}

#[cfg(not(windows))]
fn spawn_hotkey_worker() {}

/// Сообщение Windows из очереди потока (нужно для разбора WM_HOTKEY).
#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct Msg {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point_x: i32,
    point_y: i32,
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

/// Убирает окно панели с панели задач и из Alt+Tab.
fn hide_from_taskbar<T: slint::ComponentHandle + 'static>(weak: &slint::Weak<T>) {
    if let Some(window) = weak.upgrade() {
        apply_panel_style(window.window());
    }
}

/// Снимает WS_EX_APPWINDOW и ставит WS_EX_TOOLWINDOW.
#[cfg(windows)]
fn apply_panel_style(window: &slint::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let slint_handle = window.window_handle();
    let Ok(handle) = slint_handle.window_handle() else {
        log::warn!("panel: окно ещё не создано, стили не применены");
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_ref() else {
        return;
    };
    let hwnd = win32.hwnd.get();

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (ex_style & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW;
        if new_style == ex_style {
            return;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        // Панель задач пересчитывает кнопки только после скрытия и показа окна.
        ShowWindow(hwnd, SW_HIDE);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        log::info!("panel: hwnd=0x{hwnd:x}, кнопка с панели задач убрана");
    }
}

#[cfg(not(windows))]
fn apply_panel_style(_window: &slint::Window) {}

/// Разрешает бросать файлы на рабочую область.
#[cfg(windows)]
fn accept_dropped_files(hwnd: isize) {
    unsafe {
        DragAcceptFiles(hwnd, 1);
    }
}

/// Достаёт пути из `HDROP` брошенных файлов.
#[cfg(windows)]
fn dropped_paths(hdrop: usize) -> Vec<std::path::PathBuf> {
    use std::ptr;

    let count = unsafe { DragQueryFileW(hdrop as isize, u32::MAX, ptr::null_mut(), 0) };
    let mut paths = Vec::new();
    for index in 0..count {
        let length = unsafe { DragQueryFileW(hdrop as isize, index, ptr::null_mut(), 0) };
        if length == 0 {
            continue;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let written = unsafe {
            DragQueryFileW(
                hdrop as isize,
                index,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        };
        if written == 0 {
            continue;
        }
        let text = String::from_utf16_lossy(&buffer[..written as usize]);
        paths.push(std::path::PathBuf::from(text));
    }
    paths
}

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

    if msg == WM_DROPFILES {
        let paths = dropped_paths(wparam);
        if !paths.is_empty() {
            let (x, y) = win32::cursor_pos().unwrap_or((0, 0));
            action::dispatch(action::Action::Drop { x, y, paths });
        }
        unsafe { DragFinish(wparam as isize) };
        return 0;
    }

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
const WM_DROPFILES: u32 = 0x0233;
#[cfg(windows)]
const SW_HIDE: i32 = 0;
#[cfg(windows)]
const SW_SHOWNOACTIVATE: i32 = 4;
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
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn DragAcceptFiles(hwnd: isize, accept: i32);
    fn DragQueryFileW(hdrop: isize, index: u32, buffer: *mut u16, size: u32) -> u32;
    fn DragFinish(hdrop: isize);
    fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, vk: u32) -> i32;
    fn RegisterWindowMessageW(name: *const u16) -> u32;
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
