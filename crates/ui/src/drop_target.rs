//! OLE drop-target рабочей области: Проводник отдаёт файлы только через него.
//!
//! `WM_DROPFILES` остаётся запасным путём, но современный Проводник использует
//! OLE-перетаскивание и без `IDropTarget` бросок до окна не доходит.

use std::ffi::c_void;
use std::ptr;

type HResult = i32;
const S_OK: HResult = 0;
const E_NOINTERFACE: HResult = 0x8000_4002u32 as i32;
const E_FAIL: HResult = 0x8000_4005u32 as i32;

const CF_HDROP: u16 = 15;
const DVASPECT_CONTENT: u32 = 1;
const TYMED_HGLOBAL: u32 = 1;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct PointL {
    x: i32,
    y: i32,
}

#[repr(C)]
struct FormatEtc {
    format: u16,
    target_device: *const c_void,
    aspect: u32,
    index: i32,
    tymed: u32,
}

#[repr(C)]
struct StgMedium {
    tymed: u32,
    handle: isize,
    unknown: *mut c_void,
}

#[repr(C)]
struct DataObjectVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_data: unsafe extern "system" fn(*mut c_void, *const FormatEtc, *mut StgMedium) -> HResult,
}

#[repr(C)]
struct DataObject {
    vtbl: *const DataObjectVtbl,
}

#[repr(C)]
struct DropTargetVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HResult,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    drag_enter:
        unsafe extern "system" fn(*mut c_void, *mut DataObject, u32, PointL, *mut u32) -> HResult,
    drag_over: unsafe extern "system" fn(*mut c_void, u32, PointL, *mut u32) -> HResult,
    drag_leave: unsafe extern "system" fn(*mut c_void) -> HResult,
    drop: unsafe extern "system" fn(*mut c_void, *mut DataObject, u32, PointL, *mut u32) -> HResult,
}

#[repr(C)]
struct DropTarget {
    vtbl: *const DropTargetVtbl,
}

const DROPEFFECT_COPY: u32 = 1;

unsafe impl Sync for DropTarget {}

static DROP_TARGET: DropTarget = DropTarget {
    vtbl: &DropTargetVtbl {
        query_interface,
        add_ref,
        release,
        drag_enter,
        drag_over,
        drag_leave,
        drop,
    },
};

unsafe extern "system" fn query_interface(
    _this: *mut c_void,
    _id: *const Guid,
    out: *mut *mut c_void,
) -> HResult {
    if !out.is_null() {
        *out = ptr::null_mut();
    }
    E_NOINTERFACE
}

unsafe extern "system" fn add_ref(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn release(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn drag_enter(
    _this: *mut c_void,
    _data: *mut DataObject,
    _keys: u32,
    _point: PointL,
    effect: *mut u32,
) -> HResult {
    if !effect.is_null() {
        *effect = DROPEFFECT_COPY;
    }
    S_OK
}

unsafe extern "system" fn drag_over(
    _this: *mut c_void,
    _keys: u32,
    _point: PointL,
    effect: *mut u32,
) -> HResult {
    if !effect.is_null() {
        *effect = DROPEFFECT_COPY;
    }
    S_OK
}

unsafe extern "system" fn drag_leave(_this: *mut c_void) -> HResult {
    S_OK
}

unsafe extern "system" fn drop(
    _this: *mut c_void,
    data: *mut DataObject,
    _keys: u32,
    point: PointL,
    effect: *mut u32,
) -> HResult {
    if !effect.is_null() {
        *effect = DROPEFFECT_COPY;
    }
    if data.is_null() {
        return E_FAIL;
    }
    let paths = collect_paths(data);
    if paths.is_empty() {
        return E_FAIL;
    }
    crate::action::dispatch(crate::action::Action::Drop {
        x: point.x,
        y: point.y,
        paths,
    });
    S_OK
}

/// Достаёт пути из `IDataObject` (формат CF_HDROP).
unsafe fn collect_paths(data: *mut DataObject) -> Vec<std::path::PathBuf> {
    let vtbl = (*data).vtbl;
    let format = FormatEtc {
        format: CF_HDROP,
        target_device: ptr::null(),
        aspect: DVASPECT_CONTENT,
        index: -1,
        tymed: TYMED_HGLOBAL,
    };
    let mut medium = StgMedium {
        tymed: 0,
        handle: 0,
        unknown: ptr::null_mut(),
    };
    let result = ((*vtbl).get_data)(data as *mut c_void, &format, &mut medium);
    if result != S_OK || medium.handle == 0 {
        return Vec::new();
    }

    let mut paths = Vec::new();
    let hdrop = medium.handle;
    let count = DragQueryFileW(hdrop, u32::MAX, ptr::null_mut(), 0);
    for index in 0..count {
        let length = DragQueryFileW(hdrop, index, ptr::null_mut(), 0);
        if length == 0 {
            continue;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let written = DragQueryFileW(hdrop, index, buffer.as_mut_ptr(), buffer.len() as u32);
        if written == 0 {
            continue;
        }
        paths.push(std::path::PathBuf::from(String::from_utf16_lossy(
            &buffer[..written as usize],
        )));
    }
    ReleaseStgMedium(&mut medium);
    paths
}

#[link(name = "ole32")]
extern "system" {
    fn OleInitialize(reserved: *mut c_void) -> HResult;
    fn RegisterDragDrop(hwnd: isize, target: *mut c_void) -> HResult;
    fn RevokeDragDrop(hwnd: isize) -> HResult;
    fn ReleaseStgMedium(medium: *mut StgMedium);
}

#[link(name = "shell32")]
extern "system" {
    fn DragQueryFileW(hdrop: isize, index: u32, buffer: *mut u16, size: u32) -> u32;
}

/// Регистрирует рабочую область как OLE-цель перетаскивания.
pub fn install(hwnd: isize) {
    unsafe {
        // S_FALSE (1) значит, что COM уже инициализирован — это нормально.
        let _ = OleInitialize(ptr::null_mut());
        // winit регистрирует свою цель, но внешние броски в Slint 1.18 не передаёт
        // (поддержка есть только в Qt-бэкенде) — снимаем её и ставим свою.
        let revoked = RevokeDragDrop(hwnd);
        log::info!("drop: RevokeDragDrop → 0x{revoked:08x}");
        let result = RegisterDragDrop(hwnd, &DROP_TARGET as *const DropTarget as *mut c_void);
        if result == S_OK {
            log::info!("drop: рабочая область принимает файлы через OLE");
        } else {
            log::warn!("drop: RegisterDragDrop не сработал (0x{result:08x})");
        }
    }
}
