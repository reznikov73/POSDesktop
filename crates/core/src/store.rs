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

/// Сохраняет раскладку, создавая каталог при необходимости.
pub fn save(path: &Path, layout: &Layout) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = layout.to_json_pretty();
    fs::write(path, text)
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
}
