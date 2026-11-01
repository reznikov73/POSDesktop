//! POS Desktop — точка входа.

use posdesktop_ai::{class_of, policy_for, Context};
use posdesktop_core::{logging, Layout, VERSION};
use posdesktop_integrations::{DemoMail, MailSource};
use std::path::Path;

fn main() {
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
    log::info!("открываю окно (overlay={overlay})");
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
    println!("        --overlay   окно-оверлей: поверх всех, без фокуса");
    println!("    -V, --version   версия");
    println!("        --layout    раскладка по умолчанию в JSON");
    println!("        --check     проверить раскладку");
    println!("        --demo      демонстрация моделей (почта, инструменты)");
    println!("    -h, --help      эта справка");
}

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
