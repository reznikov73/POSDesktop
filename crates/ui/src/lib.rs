//! UI-слой POS Desktop на Slint.

slint::include_modules!();

/// Открывает окно приложения и запускает цикл событий.
pub fn run() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}
