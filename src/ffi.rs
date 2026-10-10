// 手写 Win32 FFI 声明。零第三方依赖，编译快、体积小、结果可控。
#![allow(dead_code, non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::ffi::c_void;

pub type HANDLE = *mut c_void;
pub type HICON = *mut c_void;
pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HFONT = *mut c_void;
pub type HMENU = *mut c_void;
pub type HINSTANCE = *mut c_void;
pub type BOOL = i32;
pub type DWORD = u32;
pub type UINT = u32;
pub type LONG = i32;
pub type ULONG = u32;
pub type WORD = u16;
pub type COLORREF = u32;
pub type LPARAM = isize;
pub type WPARAM = usize;
pub type LRESULT = isize;
pub type NTSTATUS = i32;
pub type ULONGLONG = u64;
pub type PDH_HQUERY = HANDLE;
pub type PDH_HCOUNTER = HANDLE;
pub type PDH_FMT = u32;
pub type REGSAM = u32;
pub type HKEY = HANDLE;

// ---------------- 常量 ----------------
pub const FALSE: BOOL = 0;
pub const TRUE: BOOL = 1;

pub const WM_USER: UINT = 0x0400;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_LBUTTONDBLCLK: UINT = 0x0203;
pub const WM_CONTEXTMENU: UINT = 0x007B;
pub const WM_INITMENUPOPUP: UINT = 0x0117;

pub const WS_POPUP: DWORD = 0x8000_0000;
pub const CS_HREDRAW: UINT = 0x0002;
pub const CS_VREDRAW: UINT = 0x0001;

pub const NIM_ADD: DWORD = 0;
pub const NIM_MODIFY: DWORD = 1;
pub const NIM_DELETE: DWORD = 2;
pub const NIF_MESSAGE: UINT = 0x01;
pub const NIF_ICON: UINT = 0x02;
pub const NIF_TIP: UINT = 0x04;
pub const NIF_SHOWTIP: UINT = 0x80;

pub const MF_STRING: UINT = 0x0000;
pub const MF_SEPARATOR: UINT = 0x0800;
pub const TPM_LEFTALIGN: UINT = 0x0000;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;
pub const TPM_RETURNCMD: UINT = 0x0100;

pub const TRANSPARENT: i32 = 1;
pub const FW_BOLD: i32 = 700;
pub const DEFAULT_CHARSET: DWORD = 1;
pub const OUT_DEFAULT_PRECIS: DWORD = 0;
pub const CLIP_DEFAULT_PRECIS: DWORD = 0;
pub const DEFAULT_QUALITY: DWORD = 0;
pub const DIB_RGB_COLORS: UINT = 0;

pub const IMAGE_ICON: UINT = 1;
pub const LR_DEFAULTCOLOR: UINT = 0x0000;
pub const LR_SHARED: UINT = 0x8000;

pub const SECTION_MAP_READ: DWORD = 0x0004;
pub const FILE_MAP_READ: DWORD = 0x0004;

// NtQuerySystemInformation classes
pub const SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION: u32 = 8;
pub const SYSTEM_PROCESS_INFORMATION: u32 = 5;
// PROCESSOR_POWER_INFORMATION level for CallNtPowerInformation
pub const PROCESSOR_INFORMATION_LEVEL: i32 = 11;
pub const SYSTEM_BASIC_INFORMATION: u32 = 0;

pub const PDH_FMT_DOUBLE: PDH_FMT = 0x0000_0200;

pub const KEY_READ: REGSAM = 0x0002_0019;
pub const KEY_WRITE: REGSAM = 0x0002_0006;
pub const REG_DWORD: DWORD = 4;
pub const REG_SZ: DWORD = 1;
pub const HKEY_CURRENT_USER: usize = 0x8000_0001;

pub const ATTACH_PARENT_PROCESS: DWORD = 0xFFFF_FFFF;

// ---------------- 结构体 ----------------
#[repr(C)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wparam: WPARAM,
    pub lparam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
    pub l_private: DWORD,
}

#[repr(C)]
pub struct WNDCLASSW {
    pub style: UINT,
    pub lpfn_wnd_proc: unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
    pub cb_cls_extra: i32,
    pub cb_wnd_extra: i32,
    pub h_instance: HINSTANCE,
    pub h_icon: HICON,
    pub h_cursor: HICON,
    pub hbr_background: HBRUSH,
    pub lpsz_menu_name: *const u16,
    pub lpsz_class_name: *const u16,
}

#[repr(C)]
pub struct NOTIFYICONDATAW {
    pub cb_size: DWORD,
    pub hwnd: HWND,
    pub uid: UINT,
    pub uflags: UINT,
    pub callback_message: UINT,
    pub hicon: HICON,
    pub tip: [u16; 128],
    pub state: DWORD,
    pub state_mask: DWORD,
    pub info: [u16; 256],
    pub timeout_or_version: UINT,
    pub info_title: [u16; 64],
    pub info_flags: DWORD,
    pub guid_item: [u8; 16],
    pub h_balloon_icon: HICON,
}

#[repr(C)]
pub struct BITMAPINFOHEADER {
    pub bi_size: DWORD,
    pub bi_width: LONG,
    pub bi_height: LONG,
    pub bi_planes: WORD,
    pub bi_bit_count: WORD,
    pub bi_compression: DWORD,
    pub bi_size_image: DWORD,
    pub bi_x_pels_per_meter: LONG,
    pub bi_y_pels_per_meter: LONG,
    pub bi_clr_used: DWORD,
    pub bi_clr_important: DWORD,
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmi_header: BITMAPINFOHEADER,
    pub bmi_colors: [DWORD; 1],
}

#[repr(C)]
pub struct ICONINFO {
    pub f_icon: BOOL,
    pub x_hotspot: DWORD,
    pub y_hotspot: DWORD,
    pub hbm_mask: HBITMAP,
    pub hbm_color: HBITMAP,
}

#[repr(C)]
pub struct MEMORYSTATUSEX {
    pub dw_length: DWORD,
    pub dw_memory_load: DWORD,
    pub ull_total_phys: ULONGLONG,
    pub ull_avail_phys: ULONGLONG,
    pub ull_total_page_file: ULONGLONG,
    pub ull_avail_page_file: ULONGLONG,
    pub ull_total_virtual: ULONGLONG,
    pub ull_avail_virtual: ULONGLONG,
    pub ull_avail_extended_virtual: ULONGLONG,
}

/// 每核心性能数据。注意 KernelTime 包含 IdleTime，故 busy = kernel + user - idle。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION {
    pub idle_time: ULONGLONG,
    pub kernel_time: ULONGLONG,
    pub user_time: ULONGLONG,
    pub dpc_time: ULONGLONG,
    pub interrupt_time: ULONGLONG,
    pub interrupt_count: ULONG,
    pub _reserved: ULONG,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PROCESSOR_POWER_INFORMATION {
    pub number: ULONG,
    pub max_mhz: ULONG,
    pub current_mhz: ULONG,
    pub mhz_limit: ULONG,
    pub max_idle_state: ULONG,
    pub current_idle_state: ULONG,
}

/// SYSTEM_PROCESS_INFORMATION 头部，只声明到需要用到的字段，
/// 其余用 NextEntryOffset 跳转即可，无需关心尾部线程数组布局。
#[repr(C)]
pub struct SYSTEM_PROCESS_INFORMATION {
    pub next_entry_offset: ULONG,
    pub number_of_threads: ULONG,
    pub working_set_private: ULONGLONG,
    pub hard_fault_count: ULONG,
    pub thread_high_watermark: ULONG,
    pub cycle_time: ULONGLONG,
    pub create_time: ULONGLONG,
    pub user_time: ULONGLONG,
    pub kernel_time: ULONGLONG,
    pub name_length: u16,
    pub name_max_length: u16,
    pub _pad: ULONG,
    pub name_buffer: *const u16,
    pub base_priority: LONG,
    pub process_id: ULONGLONG,
    pub parent_process_id: ULONGLONG,
    pub handle_count: ULONG,
    pub session_id: ULONG,
    pub process_key: ULONGLONG,
    pub peak_virtual_size: ULONGLONG,
    pub virtual_size: ULONGLONG,
    pub page_fault_count: ULONG,
    pub _pad2: ULONG,
    pub peak_working_set_size: ULONGLONG,
    pub working_set_size: ULONGLONG,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PDH_FMT_COUNTERVALUE {
    pub c_status: u32,
    pub value: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PDH_FMT_COUNTERVALUE_ITEM_W {
    pub sz_name: *mut u16,
    pub fmt_value: PDH_FMT_COUNTERVALUE,
}

#[repr(C)]
pub struct SYSTEMTIME {
    pub year: WORD,
    pub month: WORD,
    pub day_of_week: WORD,
    pub day: WORD,
    pub hour: WORD,
    pub minute: WORD,
    pub second: WORD,
    pub milliseconds: WORD,
}

#[repr(C)]
pub struct TIME_ZONE_INFORMATION {
    pub bias: LONG,
    pub standard_name: [u16; 32],
    pub standard_date: SYSTEMTIME,
    pub standard_bias: LONG,
    pub daylight_name: [u16; 32],
    pub daylight_date: SYSTEMTIME,
    pub daylight_bias: LONG,
}

/// HWiNFO 共享内存 v2 头部
#[repr(C)]
pub struct HWI_SENSORS_SHARED_MEM2 {
    pub signature: DWORD,
    pub version: DWORD,
    pub revision: DWORD,
    pub poll_time: ULONGLONG,
    pub offset_sensor_section: DWORD,
    pub size_sensor_section: DWORD,
    pub offset_reading_section: DWORD,
    pub size_reading_section: DWORD,
}

pub const HWI_SENSOR_ELEMENT_SIZE: usize = 264; // 4+4+128+128
pub const HWI_READING_ELEMENT_SIZE: usize = 288; // 4+4+128+128+16+8*3

extern "system" {
    // kernel32
    pub fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
    pub fn LoadLibraryW(name: *const u16) -> HINSTANCE;
    pub fn GetProcAddress(module: HINSTANCE, name: *const u8) -> *const c_void;
    pub fn CloseHandle(handle: HANDLE) -> BOOL;
    pub fn GetLastError() -> DWORD;
    pub fn Sleep(ms: DWORD);
    pub fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> BOOL;
    pub fn GlobalMemoryStatusEx(buf: *mut MEMORYSTATUSEX) -> BOOL;
    pub fn OpenFileMappingW(access: DWORD, inherit: BOOL, name: *const u16) -> HANDLE;
    pub fn MapViewOfFile(handle: HANDLE, access: DWORD, off_high: DWORD, off_low: DWORD, cap: usize) -> *mut c_void;
    pub fn UnmapViewOfFile(base: *const c_void) -> BOOL;
    pub fn QueryPerformanceCounter(counter: *mut i64) -> BOOL;
    pub fn QueryPerformanceFrequency(freq: *mut i64) -> BOOL;
    pub fn AllocConsole() -> BOOL;
    pub fn AttachConsole(pid: DWORD) -> BOOL;
    pub fn GetStdHandle(id: DWORD) -> HANDLE;
    pub fn CreateMutexW(attrs: *const c_void, initial: BOOL, name: *const u16) -> HANDLE;
    pub fn GetTimeZoneInformation(info: *mut TIME_ZONE_INFORMATION) -> DWORD;

    // user32
    pub fn RegisterClassW(class: *const WNDCLASSW) -> u16;
    pub fn CreateWindowExW(
        ex_style: DWORD, class_name: *const u16, window_name: *const u16, style: DWORD,
        x: i32, y: i32, w: i32, h: i32, parent: HWND, menu: HMENU,
        instance: HINSTANCE, param: *const c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
    pub fn GetMessageW(msg: *mut MSG, hwnd: HWND, min: UINT, max: UINT) -> BOOL;
    pub fn TranslateMessage(msg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(msg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(code: i32);
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn SetTimer(hwnd: HWND, id: usize, elapse: UINT, proc: *const c_void) -> usize;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(menu: HMENU, flags: UINT, id: usize, name: *const u16) -> BOOL;
    pub fn DestroyMenu(menu: HMENU) -> BOOL;
    pub fn TrackPopupMenu(
        menu: HMENU, flags: UINT, x: i32, y: i32, reserved: i32, hwnd: HWND, rect: *const RECT,
    ) -> BOOL;
    pub fn GetCursorPos(pt: *mut POINT) -> BOOL;
    pub fn GetDC(hwnd: HWND) -> HDC;
    pub fn ReleaseDC(hwnd: HWND, dc: HDC) -> i32;
    pub fn PostMessageW(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> BOOL;
    pub fn LoadIconW(instance: HINSTANCE, name: *const u16) -> HICON;
    pub fn DestroyIcon(icon: HICON) -> BOOL;

    // gdi32
    pub fn CreateCompatibleDC(dc: HDC) -> HDC;
    pub fn DeleteDC(dc: HDC) -> BOOL;
    pub fn CreateCompatibleBitmap(dc: HDC, w: i32, h: i32) -> HBITMAP;
    pub fn CreateBitmap(w: i32, h: i32, planes: UINT, bits: UINT, pixels: *const c_void) -> HBITMAP;
    pub fn SelectObject(dc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    pub fn DeleteObject(obj: HGDIOBJ) -> BOOL;
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn FillRect(dc: HDC, rect: *const RECT, brush: HBRUSH) -> i32;
    pub fn CreateFontW(
        h: i32, w: i32, escapement: i32, orientation: i32, weight: i32,
        italic: DWORD, underline: DWORD, strike: DWORD, charset: DWORD,
        out_precision: DWORD, clip_precision: DWORD, quality: DWORD,
        pitch_family: DWORD, face: *const u16,
    ) -> HFONT;
    pub fn SetTextColor(dc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetBkMode(dc: HDC, mode: i32) -> i32;
    pub fn TextOutW(dc: HDC, x: i32, y: i32, text: *const u16, len: i32) -> BOOL;
    pub fn DrawTextW(dc: HDC, text: *const u16, len: i32, rect: *mut RECT, format: UINT) -> i32;
    pub fn CreateIconIndirect(info: *const ICONINFO) -> HICON;

    // shell32
    pub fn Shell_NotifyIconW(cmd: DWORD, data: *mut NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(
        hwnd: HWND, op: *const u16, file: *const u16, params: *const u16,
        dir: *const u16, show: i32,
    ) -> usize;

    // advapi32
    pub fn RegOpenKeyExW(key: HKEY, sub: *const u16, opts: DWORD, access: REGSAM, out: *mut HKEY) -> i32;
    pub fn RegCreateKeyExW(
        key: HKEY, sub: *const u16, reserved: DWORD, class: *const u16,
        opts: DWORD, access: REGSAM, sec: *const c_void, out: *mut HKEY, disp: *mut DWORD,
    ) -> i32;
    pub fn RegSetValueExW(key: HKEY, name: *const u16, reserved: DWORD, ty: DWORD, data: *const u8, len: DWORD) -> i32;
    pub fn RegQueryValueExW(
        key: HKEY, name: *const u16, reserved: *const DWORD, ty: *mut DWORD, data: *mut u8, len: *mut DWORD,
    ) -> i32;
    pub fn RegDeleteValueW(key: HKEY, name: *const u16) -> i32;
    pub fn RegCloseKey(key: HKEY) -> i32;
}

// 动态加载的函数指针
pub type FnNtQuerySystemInformation =
    unsafe extern "system" fn(u32, *mut c_void, ULONG, *mut ULONG) -> NTSTATUS;
pub type FnCallNtPowerInformation =
    unsafe extern "system" fn(i32, *mut c_void, ULONG, *mut c_void, ULONG) -> NTSTATUS;
pub type FnPdhOpenQueryW = unsafe extern "system" fn(*const u16, usize, *mut PDH_HQUERY) -> i32;
pub type FnPdhAddEnglishCounterW =
    unsafe extern "system" fn(PDH_HQUERY, *const u16, usize, *mut PDH_HCOUNTER) -> i32;
pub type FnPdhCollectQueryData = unsafe extern "system" fn(PDH_HQUERY) -> i32;
pub type FnPdhGetFormattedCounterValue =
    unsafe extern "system" fn(PDH_HCOUNTER, PDH_FMT, *mut DWORD, *mut PDH_FMT_COUNTERVALUE) -> i32;
pub type FnPdhGetFormattedCounterArrayW = unsafe extern "system" fn(
    PDH_HCOUNTER, PDH_FMT, *mut DWORD, *mut DWORD, *mut PDH_FMT_COUNTERVALUE_ITEM_W,
) -> i32;
