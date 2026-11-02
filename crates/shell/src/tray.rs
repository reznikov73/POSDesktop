//! Иконка в области уведомлений с меню (ТЗ M1: трей).
//!
//! Иконка живёт в отдельном потоке с собственной оконной процедурой: меню
//! `TrackPopupMenu` блокирует поток, пока пользователь не выберет пункт.
//! Команды возвращаются через колбэк, поэтому UI-слой сам решает, что делать.

/// Команда меню трея.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Показать или скрыть панель приложений.
    TogglePanels,
    /// Отменить последнее перемещение плитки.
    Undo,
    /// Повторить отменённое перемещение.
    Redo,
    /// Открыть каталог логов.
    OpenLogs,
    /// Завершить работу оболочки.
    Quit,
}

/// Дескриптор иконки трея.
///
/// Иконка удаляется при уничтожении дескриптора: либо вручную [`TrayHandle::shutdown`],
/// либо при завершении процесса.
#[derive(Debug)]
pub struct TrayHandle {
    #[cfg(windows)]
    hwnd: isize,
}

impl TrayHandle {
    /// Убирает иконку и останавливает поток трея.
    pub fn shutdown(&self) {
        #[cfg(windows)]
        imp::shutdown(self.hwnd);
    }
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(windows)]
mod imp {
    use super::TrayCommand;
    use std::ffi::c_void;
    use std::ptr;

    /// Сообщение от иконки (WM_APP + 1).
    pub(super) const CALLBACK_MESSAGE: u32 = 0x8001;
    const WM_CLOSE: u32 = 0x0010;
    const WM_LBUTTONDBLCLK: usize = 0x0203;
    const WM_RBUTTONUP: usize = 0x0205;

    const NIM_ADD: u32 = 0;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 0x01;
    const NIF_ICON: u32 = 0x02;
    const NIF_TIP: u32 = 0x04;
    const IDI_APPLICATION: usize = 32512;

    const MF_STRING: u32 = 0x0000;
    const TPM_RIGHTBUTTON: u32 = 0x0002;
    const TPM_RETURNCMD: u32 = 0x0100;
    const TPM_NONOTIFY: u32 = 0x0080;

    const HWND_MESSAGE: isize = -3;
    const SUBCLASS_ID: usize = 4;

    const MENU_TOGGLE: usize = 1;
    const MENU_LOGS: usize = 2;
    const MENU_UNDO: usize = 3;
    const MENU_REDO: usize = 4;
    const MENU_QUIT: usize = 5;

    const ID: u32 = 1;
    const TIP: &str = "POS Desktop";

    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: isize,
        uid: u32,
        flags: u32,
        callback_message: u32,
        h_icon: isize,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        timeout_or_version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid: [u8; 16],
        balloon_icon: isize,
    }

    impl NotifyIconDataW {
        fn new(hwnd: isize) -> Self {
            Self {
                cb_size: std::mem::size_of::<Self>() as u32,
                hwnd,
                uid: ID,
                flags: 0,
                callback_message: 0,
                h_icon: 0,
                tip: [0; 128],
                state: 0,
                state_mask: 0,
                info: [0; 256],
                timeout_or_version: 0,
                info_title: [0; 64],
                info_flags: 0,
                guid: [0; 16],
                balloon_icon: 0,
            }
        }
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Msg {
        hwnd: isize,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        point_x: i32,
        point_y: i32,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *mut c_void,
        ) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostQuitMessage(code: i32);
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn LoadIconW(instance: isize, name: *const u16) -> isize;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
        fn TrackPopupMenu(
            menu: isize,
            flags: u32,
            x: i32,
            y: i32,
            reserved: i32,
            hwnd: isize,
            rect: *const c_void,
        ) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
    }

    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: isize,
            pfn_subclass: Option<
                extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize,
            >,
            id: usize,
            data: usize,
        ) -> i32;
        fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Создаёт иконку и запускает поток её сообщений.
    pub(super) fn spawn(
        on_command: impl Fn(TrayCommand) + Send + 'static,
    ) -> Option<super::TrayHandle> {
        let (ready, port) = std::sync::mpsc::channel::<isize>();

        std::thread::Builder::new()
            .name("posdesktop-tray".into())
            .spawn(move || unsafe {
                let class = wide("STATIC");
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    0,
                    0,
                    ptr::null_mut(),
                );
                if hwnd == 0 {
                    log::error!("трей: не удалось создать служебное окно");
                    let _ = ready.send(0);
                    return;
                }

                let callback: Box<dyn Fn(TrayCommand) + Send> = Box::new(on_command);
                let data = Box::into_raw(Box::new(callback)) as usize;
                SetWindowSubclass(hwnd, Some(tray_proc), SUBCLASS_ID, data);

                if !add_icon(hwnd) {
                    log::error!("трей: Shell_NotifyIcon не добавил иконку");
                    let _ = ready.send(0);
                    DestroyWindow(hwnd);
                    drop(Box::from_raw(data as *mut Box<dyn Fn(TrayCommand) + Send>));
                    return;
                }
                log::info!("трей: иконка добавлена, hwnd=0x{hwnd:x}");
                let _ = ready.send(hwnd);

                let mut msg = Msg::default();
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                remove_icon(hwnd);
                DestroyWindow(hwnd);
                drop(Box::from_raw(data as *mut Box<dyn Fn(TrayCommand) + Send>));
            })
            .ok()?;

        let hwnd = port.recv().ok()?;
        if hwnd == 0 {
            return None;
        }
        Some(super::TrayHandle { hwnd })
    }

    /// Просит поток трея завершиться: WM_CLOSE убирает иконку.
    pub(super) fn shutdown(hwnd: isize) {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    unsafe fn add_icon(hwnd: isize) -> bool {
        let mut data = NotifyIconDataW::new(hwnd);
        data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.callback_message = CALLBACK_MESSAGE;
        data.h_icon = LoadIconW(0, IDI_APPLICATION as *const u16);
        for (slot, code) in data.tip.iter_mut().zip(TIP.encode_utf16()) {
            *slot = code;
        }
        Shell_NotifyIconW(NIM_ADD, &data) != 0
    }

    unsafe fn remove_icon(hwnd: isize) {
        let data = NotifyIconDataW::new(hwnd);
        Shell_NotifyIconW(NIM_DELETE, &data);
    }

    unsafe fn dispatch(data: usize, command: TrayCommand) {
        if data == 0 {
            return;
        }
        let callback = &*(data as *const Box<dyn Fn(TrayCommand) + Send>);
        log::debug!("трей: команда {command:?}");
        callback(command);
    }

    unsafe fn show_menu(hwnd: isize, data: usize) {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return;
        }
        let toggle = wide("Показать / скрыть панели");
        let logs = wide("Логи");
        let undo = wide("Отменить перемещение");
        let redo = wide("Повторить перемещение");
        let quit = wide("Выход");
        AppendMenuW(menu, MF_STRING, MENU_TOGGLE, toggle.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_LOGS, logs.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_UNDO, undo.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_REDO, redo.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_QUIT, quit.as_ptr());

        let mut point = Point { x: 0, y: 0 };
        GetCursorPos(&mut point);
        // Без SetForegroundWindow меню не закроется по клику вне его.
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            point.x,
            point.y,
            0,
            hwnd,
            ptr::null(),
        );
        DestroyMenu(menu);

        match command as usize {
            MENU_TOGGLE => dispatch(data, TrayCommand::TogglePanels),
            MENU_LOGS => dispatch(data, TrayCommand::OpenLogs),
            MENU_UNDO => dispatch(data, TrayCommand::Undo),
            MENU_REDO => dispatch(data, TrayCommand::Redo),
            MENU_QUIT => dispatch(data, TrayCommand::Quit),
            _ => {}
        }
    }

    extern "system" fn tray_proc(
        hwnd: isize,
        msg: u32,
        wparam: usize,
        lparam: isize,
        _id: usize,
        data: usize,
    ) -> isize {
        unsafe {
            match msg {
                WM_CLOSE => {
                    remove_icon(hwnd);
                    DestroyWindow(hwnd);
                    PostQuitMessage(0);
                    return 0;
                }
                CALLBACK_MESSAGE => {
                    let event = lparam as usize;
                    if event == WM_LBUTTONDBLCLK {
                        dispatch(data, TrayCommand::TogglePanels);
                    } else if event == WM_RBUTTONUP {
                        show_menu(hwnd, data);
                    }
                    return 0;
                }
                _ => {}
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
    }
}

#[cfg(not(windows))]
pub fn spawn(_on_command: impl Fn(TrayCommand) + Send + 'static) -> Option<TrayHandle> {
    None
}

#[cfg(windows)]
pub fn spawn(on_command: impl Fn(TrayCommand) + Send + 'static) -> Option<TrayHandle> {
    imp::spawn(on_command)
}
