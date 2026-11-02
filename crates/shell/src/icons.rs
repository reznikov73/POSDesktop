//! Иконки приложений из Shell: ярлык из меню «Пуск» → настоящая иконка (ТЗ §5.2).
//!
//! Основной путь — `IShellItemImageFactory`: он разворачивает ярлык и отдаёт
//! иконку приложения (а не значок ссылки). Если COM не сработал, берём
//! `SHGetFileInfo`, он даёт хотя бы значок файла. Любая ошибка — `None`, плитка
//! покажет монограмму.

use std::path::{Path, PathBuf};

/// Пиксели иконки в RGBA8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Ищет ярлык приложения в меню «Пуск» и загружает его иконку.
pub fn icon_for_app(name: &str, size: u32) -> Option<IconPixels> {
    let shortcut = find_shortcut(name)?;
    load_app_icon(&shortcut, size)
}

/// Иконка приложения по ярлыку или exe.
pub fn load_app_icon(path: &Path, size: u32) -> Option<IconPixels> {
    #[cfg(windows)]
    {
        if let Some(pixels) = shell_icon(path, size) {
            return Some(pixels);
        }
    }
    load_icon(path, size)
}

/// Ярлык приложения по названию (точное совпадение важнее частичного).
pub fn find_shortcut(name: &str) -> Option<PathBuf> {
    let wanted = normalize(name);
    if wanted.is_empty() {
        return None;
    }
    let index = index();
    if let Some((_, path)) = index.iter().find(|(stem, _)| *stem == wanted) {
        return Some(path.clone());
    }
    if wanted.chars().count() < 4 {
        // «1С» подошло бы к «1С-Коннект» — коротким именам верим только точно.
        return None;
    }
    index
        .iter()
        .find(|(stem, _)| stem.starts_with(&wanted))
        .map(|(_, path)| path.clone())
}

/// Нормализация названия: нижний регистр, только буквы с цифрами,
/// латинские двойники приведены к кириллице.
///
/// «1С:Предприятие» (кириллица) и «1C Предприятие.lnk» (латиница) дают одну строку.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|symbol| symbol.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .map(homoglyph)
        .collect()
}

/// Латинские буквы, неотличимые от кириллических, приводим к кириллице.
fn homoglyph(symbol: char) -> char {
    match symbol {
        'a' => 'а',
        'b' => 'в',
        'c' => 'с',
        'e' => 'е',
        'h' => 'н',
        'k' => 'к',
        'm' => 'м',
        'o' => 'о',
        'p' => 'р',
        't' => 'т',
        'x' => 'х',
        'y' => 'у',
        other => other,
    }
}

fn index() -> &'static Vec<(String, PathBuf)> {
    use std::sync::OnceLock;

    static INDEX: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut found = Vec::new();
        for dir in start_menu_dirs() {
            collect_shortcuts(&dir, 0, &mut found);
        }
        found.sort();
        found
    })
}

fn start_menu_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for base in [roaming_appdata(), program_data()].into_iter().flatten() {
        let dir = base
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        if dir.is_dir() {
            dirs.push(dir);
        }
    }
    dirs
}

/// `%APPDATA%`; если переменной нет (служебные окружения), берём профиль.
fn roaming_appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(|profile| PathBuf::from(profile).join("AppData").join("Roaming"))
    })
}

/// `%PROGRAMDATA%`; запасной путь — `<системный диск>:\\ProgramData`.
fn program_data() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("PROGRAMDATA") {
        return Some(PathBuf::from(dir));
    }
    if let Some(drive) = std::env::var_os("SystemDrive") {
        // SystemDrive приходит как «C:», поэтому склеиваем вручную: join дал бы «C:ProgramData».
        let drive = drive.to_string_lossy();
        let dir = PathBuf::from(format!("{drive}\\ProgramData"));
        if dir.is_dir() {
            return Some(dir);
        }
    }
    let fallback = PathBuf::from("C:\\ProgramData");
    fallback.is_dir().then_some(fallback)
}

fn collect_shortcuts(dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    if depth > 5 || out.len() > 4000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, depth + 1, out);
            continue;
        }
        let is_lnk = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"));
        if !is_lnk {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        out.push((normalize(stem), path));
    }
}

/// Иконка через `IShellItemImageFactory`: разворачивает ярлык до приложения.
#[cfg(windows)]
pub fn shell_icon(path: &Path, size: u32) -> Option<IconPixels> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use com::*;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let side = size.max(16) as i32;

    unsafe {
        // RPC_E_CHANGED_MODE означает, что COM уже инициализирован иначе — это не беда.
        let _ = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);

        let mut factory: *mut c_void = ptr::null_mut();
        let hr = SHCreateItemFromParsingName(
            wide.as_ptr(),
            ptr::null_mut(),
            &IID_ISHELL_ITEM_IMAGE_FACTORY,
            &mut factory,
        );
        if hr != S_OK || factory.is_null() {
            return None;
        }

        let vtbl = *(factory as *const *const ImageFactoryVtbl);
        let mut bitmap: isize = 0;
        let hr = ((*vtbl).get_image)(
            factory,
            Size { cx: side, cy: side },
            SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            &mut bitmap,
        );
        ((*vtbl).release)(factory);
        if hr != S_OK || bitmap == 0 {
            return None;
        }

        let pixels = bitmap_to_rgba(bitmap);
        DeleteObject(bitmap);
        pixels
    }
}

#[cfg(not(windows))]
pub fn shell_icon(_path: &Path, _size: u32) -> Option<IconPixels> {
    None
}

#[cfg(windows)]
mod com {
    use std::ffi::c_void;

    pub const S_OK: i32 = 0;
    pub const COINIT_APARTMENTTHREADED: u32 = 0x2;
    pub const SIIGBF_BIGGERSIZEOK: u32 = 0x0000_0001;
    pub const SIIGBF_ICONONLY: u32 = 0x0000_0004;

    #[repr(C)]
    pub struct Guid {
        pub data1: u32,
        pub data2: u16,
        pub data3: u16,
        pub data4: [u8; 8],
    }

    pub const IID_ISHELL_ITEM_IMAGE_FACTORY: Guid = Guid {
        data1: 0xbcc1_8b79,
        data2: 0xba16,
        data3: 0x442f,
        data4: [0x80, 0xc4, 0x8a, 0x59, 0xc3, 0x0c, 0x46, 0x3b],
    };

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Size {
        pub cx: i32,
        pub cy: i32,
    }

    pub type QueryInterfaceFn =
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
    pub type AddRefFn = unsafe extern "system" fn(*mut c_void) -> u32;
    pub type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
    pub type GetImageFn = unsafe extern "system" fn(*mut c_void, Size, u32, *mut isize) -> i32;

    #[repr(C)]
    pub struct ImageFactoryVtbl {
        pub query_interface: QueryInterfaceFn,
        pub add_ref: AddRefFn,
        pub release: ReleaseFn,
        pub get_image: GetImageFn,
    }

    #[link(name = "ole32")]
    extern "system" {
        pub fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        pub fn SHCreateItemFromParsingName(
            path: *const u16,
            bind_context: *mut c_void,
            riid: *const Guid,
            out: *mut *mut c_void,
        ) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn GetObjectW(object: isize, size: i32, out: *mut c_void) -> i32;
        pub fn GetDIBits(
            dc: isize,
            bitmap: isize,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        pub fn DeleteObject(object: isize) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetDC(hwnd: isize) -> isize;
        pub fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
    }

    #[repr(C)]
    pub struct BitmapInfoHeader {
        pub size: u32,
        pub width: i32,
        pub height: i32,
        pub planes: u16,
        pub bit_count: u16,
        pub compression: u32,
        pub size_image: u32,
        pub x_pixels_per_meter: i32,
        pub y_pixels_per_meter: i32,
        pub colors_used: u32,
        pub colors_important: u32,
    }

    #[repr(C)]
    pub struct BitmapInfo {
        pub header: BitmapInfoHeader,
        pub colors: [u32; 1],
    }
}

/// Читает `HBITMAP` в RGBA. Строки в DIB идут снизу вверх.
#[cfg(windows)]
unsafe fn bitmap_to_rgba(bitmap: isize) -> Option<IconPixels> {
    use com::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct Bitmap {
        kind: i32,
        width: i32,
        height: i32,
        width_bytes: i32,
        planes: u16,
        bits_pixel: u16,
        bits: *mut c_void,
    }

    let mut header: Bitmap = std::mem::zeroed();
    let size = std::mem::size_of::<Bitmap>() as i32;
    if GetObjectW(bitmap, size, &mut header as *mut Bitmap as *mut c_void) == 0 {
        return None;
    }
    let (width, height) = (header.width, header.height);
    if width <= 0 || height <= 0 {
        return None;
    }

    let mut info = BitmapInfo {
        header: BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width,
            height,
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        },
        colors: [0],
    };

    let mut buffer = vec![0u8; (width * height * 4) as usize];
    let dc = GetDC(0);
    let lines = GetDIBits(
        dc,
        bitmap,
        0,
        height as u32,
        buffer.as_mut_ptr() as *mut c_void,
        &mut info,
        0,
    );
    ReleaseDC(0, dc);
    if lines == 0 {
        return None;
    }

    let mut rgba = Vec::with_capacity(buffer.len());
    let mut opaque = false;
    for row in (0..height).rev() {
        let start = (row * width * 4) as usize;
        for pixel in buffer[start..start + width as usize * 4].as_chunks::<4>().0 {
            let (b, g, r, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
            opaque |= a != 0;
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    if !opaque {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                pixel[3] = 255;
            }
        }
    }

    Some(IconPixels {
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

/// Запасной путь: иконка файла через `SHGetFileInfoW`.
#[cfg(windows)]
pub fn load_icon(path: &Path, size: u32) -> Option<IconPixels> {
    use ffi::*;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = ShFileInfo::default();
    let result = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<ShFileInfo>() as u32,
            SHGFI_ICON,
        )
    };
    if result == 0 || info.h_icon == 0 {
        return None;
    }
    let pixels = unsafe { icon_to_rgba(info.h_icon, size) };
    unsafe { DestroyIcon(info.h_icon) };
    pixels
}

#[cfg(not(windows))]
pub fn load_icon(_path: &Path, _size: u32) -> Option<IconPixels> {
    None
}

#[cfg(windows)]
mod ffi {
    #[repr(C)]
    pub struct ShFileInfo {
        pub h_icon: isize,
        pub i_icon: i32,
        pub attributes: u32,
        pub display_name: [u16; 260],
        pub type_name: [u16; 80],
    }

    impl Default for ShFileInfo {
        fn default() -> Self {
            Self {
                h_icon: 0,
                i_icon: 0,
                attributes: 0,
                display_name: [0; 260],
                type_name: [0; 80],
            }
        }
    }

    #[repr(C)]
    pub struct BitmapInfoHeader {
        pub size: u32,
        pub width: i32,
        pub height: i32,
        pub planes: u16,
        pub bit_count: u16,
        pub compression: u32,
        pub size_image: u32,
        pub x_pixels_per_meter: i32,
        pub y_pixels_per_meter: i32,
        pub colors_used: u32,
        pub colors_important: u32,
    }

    #[repr(C)]
    pub struct BitmapInfo {
        pub header: BitmapInfoHeader,
        pub colors: [u32; 1],
    }

    #[link(name = "shell32")]
    extern "system" {
        pub fn SHGetFileInfoW(
            path: *const u16,
            attributes: u32,
            info: *mut ShFileInfo,
            info_size: u32,
            flags: u32,
        ) -> usize;
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn DestroyIcon(icon: isize) -> i32;
        pub fn GetDC(hwnd: isize) -> isize;
        pub fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
        pub fn DrawIconEx(
            dc: isize,
            x: i32,
            y: i32,
            icon: isize,
            width: i32,
            height: i32,
            step: u32,
            brush: isize,
            flags: u32,
        ) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        pub fn CreateCompatibleDC(dc: isize) -> isize;
        pub fn DeleteDC(dc: isize) -> i32;
        pub fn SelectObject(dc: isize, object: isize) -> isize;
        pub fn DeleteObject(object: isize) -> i32;
        pub fn CreateDIBSection(
            dc: isize,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut core::ffi::c_void,
            section: isize,
            offset: u32,
        ) -> isize;
    }

    pub const SHGFI_ICON: u32 = 0x0000_0100;
    pub const DI_NORMAL: u32 = 0x0003;
}

/// Рисует иконку в 32-битный DIB и вычитывает пиксели.
#[cfg(windows)]
unsafe fn icon_to_rgba(icon: isize, size: u32) -> Option<IconPixels> {
    use ffi::*;
    use std::ptr;

    let side = size.max(16) as i32;
    let header = BitmapInfoHeader {
        size: std::mem::size_of::<BitmapInfoHeader>() as u32,
        width: side,
        height: -side, // top-down: строки сверху вниз
        planes: 1,
        bit_count: 32,
        compression: 0,
        size_image: 0,
        x_pixels_per_meter: 0,
        y_pixels_per_meter: 0,
        colors_used: 0,
        colors_important: 0,
    };
    let info = BitmapInfo {
        header,
        colors: [0],
    };

    let screen = GetDC(0);
    if screen == 0 {
        return None;
    }
    let mut bits: *mut core::ffi::c_void = ptr::null_mut();
    let dib = CreateDIBSection(screen, &info, 0, &mut bits, 0, 0);
    if dib == 0 || bits.is_null() {
        ReleaseDC(0, screen);
        return None;
    }

    let dc = CreateCompatibleDC(screen);
    let previous = SelectObject(dc, dib);
    let byte_count = (side * side * 4) as usize;
    ptr::write_bytes(bits as *mut u8, 0, byte_count);
    DrawIconEx(dc, 0, 0, icon, side, side, 0, 0, DI_NORMAL);

    let raw = std::slice::from_raw_parts(bits as *const u8, byte_count);
    let mut rgba = Vec::with_capacity(byte_count);
    let mut opaque = false;
    for pixel in raw.as_chunks::<4>().0 {
        let (b, g, r, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        opaque |= a != 0;
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    if !opaque {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
                pixel[3] = 255;
            }
        }
    }

    SelectObject(dc, previous);
    DeleteDC(dc);
    DeleteObject(dib);
    ReleaseDC(0, screen);

    Some(IconPixels {
        width: side as u32,
        height: side as u32,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ignores_case_and_punctuation() {
        assert_eq!(normalize("1С:Предприятие"), "1спредприятие");
        assert_eq!(normalize("1С Предприятие"), "1спредприятие");
        assert_eq!(normalize("VS Code"), normalize("VS Cоde"));
    }

    #[test]
    fn latin_and_cyrillic_names_match() {
        assert_eq!(normalize("1C Предприятие"), normalize("1С Предприятие"));
        assert_eq!(normalize("Excel"), normalize("Ехсеl"));
    }

    #[test]
    fn missing_app_gives_none() {
        assert_eq!(find_shortcut("Этого-Приложения-Нет-12345"), None);
    }

    #[test]
    fn known_app_resolves_if_installed() {
        // На машинах без Word ярлыка нет — тест не должен падать.
        if let Some(path) = find_shortcut("Word") {
            assert!(path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")));
        }
    }
}
