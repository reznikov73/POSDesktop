//! Загрузка и сохранение раскладки в `layout.json`.

use crate::layout::Layout;
use std::fs;
use std::path::{Path, PathBuf};

/// Путь по умолчанию: `%APPDATA%\POSDesktop\layout.json`.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|profile| PathBuf::from(profile).join("AppData").join("Roaming"))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("POSDesktop").join("layout.json")
}

/// Читает раскладку; при ошибке или отсутствии файла возвращает заводскую.
pub fn load_or_default(path: &Path) -> Layout {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Layout>(&text) {
            Ok(layout) => {
                if layout.validate().is_ok() {
                    layout
                } else {
                    log::warn!("layout.json не прошёл валидацию, берём заводскую");
                    Layout::default_primary()
                }
            }
            Err(err) => {
                log::warn!("layout.json не разобран ({err}), берём заводскую");
                Layout::default_primary()
            }
        },
        Err(err) => {
            log::info!("layout.json не найден ({err}), создаём заводскую");
            Layout::default_primary()
        }
    }
}

/// Сохраняет раскладку атомарно: временный файл и переименование.
///
/// Невалидная раскладка не пишется — на диске остаётся прежняя версия (ТЗ §9.3).
pub fn save(path: &Path, layout: &Layout) -> std::io::Result<()> {
    if let Err(err) = layout.validate() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            err.to_string(),
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = layout.to_json_pretty();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push("posdesktop-tests");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    #[test]
    fn round_trip() {
        let path = temp_path("layout-roundtrip.json");
        let layout = Layout::default_primary();
        save(&path, &layout).expect("save");
        let loaded = load_or_default(&path);
        assert_eq!(layout, loaded);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file_gives_default() {
        let path = temp_path("layout-missing.json");
        let _ = fs::remove_file(&path);
        let loaded = load_or_default(&path);
        assert_eq!(loaded, Layout::default_primary());
    }

    #[test]
    fn broken_file_gives_default() {
        let path = temp_path("layout-broken.json");
        fs::write(&path, "{ not json").expect("write");
        let loaded = load_or_default(&path);
        assert_eq!(loaded, Layout::default_primary());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn moved_layout_is_saved_and_loaded() {
        let path = temp_path("layout-moved.json");
        let mut layout = Layout::default_primary();
        {
            let screen = layout.monitors.get_mut("primary").expect("screen");
            screen.move_object("weather", 400, 400).expect("move");
        }
        save(&path, &layout).expect("save");
        assert_eq!(load_or_default(&path), layout);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn invalid_layout_is_not_written() {
        let path = temp_path("layout-invalid.json");
        let layout = Layout::default_primary();
        save(&path, &layout).expect("save");
        let before = fs::read_to_string(&path).expect("read");

        let mut broken = layout.clone();
        broken.monitors.get_mut("primary").expect("screen").objects[0]
            .rect
            .x = 18;
        let err = save(&path, &broken).expect_err("валидация должна отклонить запись");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_to_string(&path).expect("read"), before);
        let _ = fs::remove_file(&path);
    }
}
