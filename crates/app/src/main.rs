//! POS Desktop — точка входа.

use posdesktop_ai::{class_of, policy_for, Context};
use posdesktop_core::{logging, Layout, VERSION};
use posdesktop_integrations::{DemoMail, MailSource};
use std::path::Path;

fn main() {
    // Программный рендер по умолчанию: 18.8 МБ против 63.5 МБ у GPU-рендера.
    // Переопределяется переменной SLINT_BACKEND (например, winit-femtovg).
    if std::env::var_os("SLINT_BACKEND").is_none() {
        std::env::set_var("SLINT_BACKEND", "winit-software");
    }

    let log_path = logging::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    log::info!("POS Desktop {VERSION} запуск, аргументы: {args:?}");
    log::info!("лог: {}", log_path.display());

    let overlay = args.iter().any(|arg| arg == "--overlay");

    let result = match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("POS Desktop {VERSION}");
            Ok(())
        }
        Some("--layout") => {
            println!("{}", Layout::default_primary().to_json_pretty());
            Ok(())
        }
        Some("--check") => layout_check(),
        Some("--demo") => {
            demo();
            Ok(())
        }
        Some("--snapshot") => {
            posdesktop_ui::run_snapshot().map_err(|err| format!("snapshot не удался: {err}"))
        }
        Some("--clickthrough-test") => posdesktop_ui::run_clickthrough_test()
            .map_err(|err| format!("clickthrough-тест не удался: {err}")),
        Some("--workspace-snapshot") => posdesktop_ui::run_workspace_snapshot()
            .map_err(|err| format!("workspace-снапшот не удался: {err}")),
        Some("--logs") => {
            open_logs(&log_path);
            Ok(())
        }
        Some("-h") | Some("--help") => {
            help();
            Ok(())
        }
        _ => run_ui(overlay),
    };

    if let Err(message) = result {
        log::error!("{message}");
        show_error(&message, &log_path);
        log::logger().flush();
        std::process::exit(1);
    }

    log::info!("выход");
}

fn layout_check() -> Result<(), String> {
    match Layout::default_primary().validate() {
        Ok(()) => {
            println!("layout ok (version {})", posdesktop_core::LAYOUT_VERSION);
            Ok(())
        }
        Err(err) => Err(format!("layout error: {err}")),
    }
}

fn run_ui(overlay: bool) -> Result<(), String> {
    if !claim_single_instance() {
        log::info!("оболочка уже запущена — попросил её показать панель");
        return Ok(());
    }
    log::info!("открываю окно (overlay={overlay})");
    hide_console();
    posdesktop_ui::run(overlay).map_err(|err| format!("не удалось открыть окно: {err}"))
}

fn demo() {
    let mail = DemoMail::new(&[("work", 5), ("personal", 0), ("projects", 7)]);
    println!("mail: {} unread", mail.unread_total());
    for (name, count) in mail.unread_by_box() {
        println!("  {name}: {count}");
    }

    println!("ai tools: {}", posdesktop_ai::TOOLS.len());
    for (name, _) in posdesktop_ai::TOOLS.iter().take(3) {
        println!("  {name}: {:?}", class_of(name));
    }
    println!(
        "files.delete outside work folder: {:?}",
        policy_for("files.delete", Context::default())
    );
}

fn help() {
    println!("POS Desktop {VERSION}");
    println!();
    println!("Без аргументов открывает окно приложения.");
    println!();
    println!("USAGE:");
    println!("    posdesktop [OPTION]");
    println!();
    println!("OPTIONS:");
    println!("        --overlay            окно-оверлей: поверх всех, без фокуса");
    println!("        --snapshot           снапшот панели и статистика цвета");
    println!("        --workspace-snapshot снапшот рабочей области (прозрачность)");
    println!("        --clickthrough-test  проверить клик-сквозь");
    println!("        --logs               показать путь к логам и открыть папку");
    println!("    -V, --version            версия");
    println!("        --layout             раскладка по умолчанию в JSON");
    println!("        --check              проверить раскладку");
    println!("        --demo               демонстрация моделей (почта, инструменты)");
    println!("    -h, --help               эта справка");
}

fn open_logs(log_path: &Path) {
    println!("{}", log_path.display());
    if let Some(dir) = log_path.parent() {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("explorer").arg(dir).spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = dir;
        }
    }
}

/// Захватывает единственный экземпляр оболочки.
///
/// Повторный запуск не создаёт второе окно, а просит работающую оболочку
/// показать панель. Мьютекс живёт до конца процесса.
#[cfg(windows)]
fn claim_single_instance() -> bool {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> isize;
        fn GetLastError() -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterWindowMessageW(name: *const u16) -> u32;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    }

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const HWND_BROADCAST: isize = 0xFFFF;

    let name: Vec<u16> = "Local\\POSDesktop.SingleInstance\0"
        .encode_utf16()
        .collect();
    let show: Vec<u16> = "POSDesktop.Show\0".encode_utf16().collect();

    unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if mutex == 0 {
            log::warn!("мьютекс единственного экземпляра не создан");
            return true;
        }
        if GetLastError() != ERROR_ALREADY_EXISTS {
            return true;
        }
        let message = RegisterWindowMessageW(show.as_ptr());
        if message != 0 {
            PostMessageW(HWND_BROADCAST, message, 0, 0);
        }
        false
    }
}

#[cfg(not(windows))]
fn claim_single_instance() -> bool {
    true
}

/// Скрывает консольное окно в GUI-режиме.
///
/// Консоль прячется, только если она целиком принадлежит процессу: при запуске
/// из терминала (`cargo run`, VS Code) в ней живут другие процессы, и скрывать
/// её нельзя — иначе исчезнет окно терминала разработчика.
#[cfg(windows)]
fn hide_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> isize;
        fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    }

    const SW_HIDE: i32 = 0;

    unsafe {
        let console = GetConsoleWindow();
        if console == 0 {
            return;
        }
        let mut list = [0u32; 4];
        let attached = GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32);
        if attached == 1 {
            ShowWindow(console, SW_HIDE);
            log::info!("консоль скрыта (GUI-режим)");
        }
    }
}

#[cfg(not(windows))]
fn hide_console() {}

#[cfg(windows)]
fn show_error(message: &str, log_path: &Path) {
    let text = format!("{message}\n\nЛог: {}", log_path.display());
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "POS Desktop"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, flags: u32) -> i32;
    }

    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0x10);
    }
}

#[cfg(not(windows))]
fn show_error(message: &str, log_path: &Path) {
    eprintln!("{message}");
    eprintln!("лог: {}", log_path.display());
}
