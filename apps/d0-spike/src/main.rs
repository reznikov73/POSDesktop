//! D0: проверка зон активации на реальном экране.
//!
//! Запуск: cargo run -p posdesktop-d0-spike
//! Наведите курсор к краям экрана — в консоли появятся события.

#[cfg(windows)]
mod win {
    #[repr(C)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetCursorPos(point: *mut Point) -> i32;
        pub fn GetSystemMetrics(index: i32) -> i32;
    }

    pub const SM_CXSCREEN: i32 = 0;
    pub const SM_CYSCREEN: i32 = 1;
}

fn main() {
    #[cfg(windows)]
    run();
    #[cfg(not(windows))]
    println!("D0 spike: только Windows");
}

#[cfg(windows)]
fn run() {
    use posdesktop_shell::{edge_at, ScreenRect, TriggerConfig};

    let width = unsafe { win::GetSystemMetrics(win::SM_CXSCREEN) };
    let height = unsafe { win::GetSystemMetrics(win::SM_CYSCREEN) };
    let cfg = TriggerConfig::default();
    let screen = ScreenRect {
        x: 0,
        y: 0,
        w: width,
        h: height,
    };

    println!(
        "screen: {width}x{height}, zone {} px, corners {} px",
        cfg.zone_px, cfg.corner_px
    );
    println!("наведите курсор к краям в течение 8 секунд...");

    let started = std::time::Instant::now();
    let mut previous = None;
    while started.elapsed() < std::time::Duration::from_secs(8) {
        let mut point = win::Point { x: 0, y: 0 };
        let ok = unsafe { win::GetCursorPos(&mut point) };
        if ok != 0 {
            let edge = edge_at((point.x, point.y), screen, &cfg);
            if edge != previous {
                println!(
                    "t={:>5} ms  cursor=({:>5},{:>5})  edge={:?}",
                    started.elapsed().as_millis(),
                    point.x,
                    point.y,
                    edge
                );
                previous = edge;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(33));
    }

    println!("готово: {previous:?}");
    println!("память процесса: Get-Process posdesktop-d0-spike | Select-Object WorkingSet64");
}
