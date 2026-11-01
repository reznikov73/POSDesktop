fn main() -> Result<(), slint::PlatformError> {
    let overlay = std::env::args().any(|arg| arg == "--overlay");
    posdesktop_ui::run(overlay)
}
