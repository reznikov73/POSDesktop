//! Логирование в `logs/app.log` рядом с проектом.
//!
//! Порядок поиска каталога:
//! 1. `POSDESKTOP_LOG_DIR` — если задан;
//! 2. корень проекта (ближайший `Cargo.toml` выше exe);
//! 3. каталог exe;
//! 4. `%APPDATA%\POSDesktop\logs` (или `%USERPROFILE%\AppData\Roaming`);
//! 5. системный temp.
//!
//! Уровень задаётся переменной `POSDESKTOP_LOG`:
//! `error`, `warn`, `info` (по умолчанию), `debug`, `trace`.

use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
const KEEP_FILES: u32 = 5;

/// Каталог логов по правилам выше.
pub fn log_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("POSDESKTOP_LOG_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(root) = project_root() {
        return root.join("logs");
    }
    if let Some(dir) = exe_dir() {
        return dir.join("logs");
    }
    appdata_logs()
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Ищет корень проекта: ближайший каталог с `Cargo.toml` вверх по дереву.
fn project_root() -> Option<PathBuf> {
    let mut dir = exe_dir()?;
    for _ in 0..6 {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

fn appdata_logs() -> PathBuf {
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
    let level = level_from_env();
    let mut candidates = vec![log_dir()];
    candidates.push(appdata_logs());
    candidates.push(std::env::temp_dir().join("POSDesktop").join("logs"));

    for dir in candidates {
        if fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let path = dir.join("app.log");
        let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
            continue;
        };
        rotate(&path);
        let logger = Logger {
            file: Mutex::new(Some(file)),
            level,
        };
        let _ = log::set_boxed_logger(Box::new(logger));
        log::set_max_level(level);
        return path;
    }

    // Не удалось открыть ни один файл — пишем только в stderr.
    let logger = Logger {
        file: Mutex::new(None),
        level,
    };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(level);
    std::env::temp_dir()
        .join("POSDesktop")
        .join("logs")
        .join("app.log")
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
