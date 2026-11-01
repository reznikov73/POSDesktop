//! Логирование в `%APPDATA%\POSDesktop\logs\app.log`.
//!
//! Уровень задаётся переменной `POSDESKTOP_LOG`:
//! `error`, `warn`, `info` (по умолчанию), `debug`, `trace`.

use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Порог ротации одного файла.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
/// Сколько поколений хранить.
const KEEP_FILES: u32 = 5;

/// Каталог логов.
pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|profile| PathBuf::from(profile).join("AppData").join("Roaming"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("POSDesktop").join("logs")
}

/// Инициализирует логгер и возвращает путь к файлу.
pub fn init() -> PathBuf {
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("app.log");
    rotate(&path);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    let level = level_from_env();

    let logger = Logger {
        file: Mutex::new(file),
        level,
    };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(level);

    path
}

fn level_from_env() -> LevelFilter {
    match std::env::var("POSDESKTOP_LOG")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn rotate(path: &Path) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }
    for index in (1..KEEP_FILES).rev() {
        let from = path.with_extension(format!("log.{index}"));
        let to = path.with_extension(format!("log.{}", index + 1));
        let _ = fs::rename(from, to);
    }
    let _ = fs::rename(path, path.with_extension("log.1"));
}

struct Logger {
    file: Mutex<Option<File>>,
    level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let line = format!(
            "{timestamp} {:<5} {}: {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        eprint!("{line}");
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.flush();
            }
        }
    }
}
