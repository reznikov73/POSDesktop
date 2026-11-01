//! POS Desktop — точка входа.

use posdesktop_ai::{class_of, policy_for, Context};
use posdesktop_core::{Layout, VERSION};
use posdesktop_integrations::{DemoMail, MailSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => println!("POS Desktop {VERSION}"),
        Some("--layout") => println!("{}", Layout::default_primary().to_json_pretty()),
        Some("--check") => match Layout::default_primary().validate() {
            Ok(()) => println!("layout ok (version {})", posdesktop_core::LAYOUT_VERSION),
            Err(err) => {
                eprintln!("layout error: {err}");
                std::process::exit(1);
            }
        },
        Some("--demo") => demo(),
        _ => help(),
    }
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
    println!("USAGE:");
    println!("    posdesktop [OPTION]");
    println!();
    println!("OPTIONS:");
    println!("    -V, --version   версия");
    println!("        --layout    раскладка по умолчанию в JSON");
    println!("        --check     проверить раскладку");
    println!("        --demo      демонстрация моделей (почта, инструменты)");
    println!("    -h, --help      эта справка");
}
