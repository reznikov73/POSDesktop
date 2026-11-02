//! Запуск приложений через Shell (ТЗ §5.2): ярлык или exe открывается как из Проводника.

use std::path::Path;

/// Открывает файл или ярлык так же, как двойной клик в Проводнике.
#[cfg(windows)]
pub fn open(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }

    const SW_SHOWNORMAL: i32 = 1;

    let file: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let operation: Vec<u16> = "open\0".encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            0,
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute возвращает значение больше 32 при успехе.
    result > 32
}

#[cfg(not(windows))]
pub fn open(_path: &Path) -> bool {
    false
}
