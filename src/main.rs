// 发布版不弹控制台窗口，只在托盘运行；需要看输出时加 --console / --once / --diag
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod attrib;
mod collect;
mod dashboard;
mod ffi;
mod web;

use crate::attrib::TzOffset;
use crate::collect::{Collector, Sample};
use crate::ffi::*;
use std::collections::VecDeque;
use std::io::Write;
use std::mem;
use std::process::Command;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU16, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_HIST: usize = 3600; // 1 秒 1 点，保留 1 小时
const MENU_OPEN: usize = 1001;
const MENU_LOGS: usize = 1002;
const MENU_AUTOSTART: usize = 1003;
const MENU_EXIT: usize = 1004;
const TRAY_MSG: UINT = WM_USER + 1;
const TIMER_ID: usize = 1;

static APP: OnceLock<Arc<Mutex<App>>> = OnceLock::new();
static PORT: AtomicU16 = AtomicU16::new(8756);
static LAST_HICON: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());
static LAST_ICON_KEY: OnceLock<Mutex<Option<(i32, i32)>>> = OnceLock::new();

pub struct App {
    pub collector: Collector,
    pub hist: VecDeque<Sample>,
    pub attrib: attrib::AttribEngine,
    pub tz: TzOffset,
    pub start_ts: i64,
    pub log_dir: std::path::PathBuf,
    pub log_file: Option<std::fs::File>,
    pub log_rows: u32,
    pub ticks: u64,
}

impl App {
    pub fn new(c: Collector) -> App {
        let dir = std::env::var("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("ThermalMonitor")
            .join("logs");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("README.txt"), LOG_README);
        App {
            collector: c,
            hist: VecDeque::with_capacity(MAX_HIST + 8),
            attrib: attrib::AttribEngine::new(),
            tz: TzOffset::detect(),
            start_ts: now_ts(),
            log_dir: dir,
            log_file: None,
            log_rows: 0,
            ticks: 0,
        }
    }

    pub fn started_elapsed_secs(&self) -> i64 {
        now_ts() - self.start_ts
    }

    pub fn collector_ncores(&self) -> usize {
        self.collector.ncores
    }

    pub fn log_status(&self) -> String {
        format!(
            "{} 行已写入 {}",
            self.log_rows,
            self.log_dir.to_string_lossy()
        )
    }

    fn ensure_log(&mut self) {
        if self.log_file.is_none() {
            let name = local_date(&self.tz) + ".csv";
            let p = self.log_dir.join(name);
            let exists = p.exists();
            match std::fs::OpenOptions::new().create(true).append(true).open(p) {
                Ok(mut f) => {
                    if !exists {
                        let _ = writeln!(
                            f,
                            "时间,温度℃,功耗W,CPU%,用户%,内核%,DPC%,中断%,平均频率MHz,内存%,磁盘KB/s,网络KB/s,Top进程"
                        );
                    }
                    self.log_file = Some(f);
                }
                Err(_) => {}
            }
        }
    }

    fn write_row(&mut self, s: &Sample) {
        self.ensure_log();
        let top = s
            .procs
            .iter()
            .take(3)
            .map(|p| format!("{}={:.1}", p.0, p.1))
            .collect::<Vec<_>>()
            .join(";");
        let line = format!(
            "{},{},{},{:.1},{:.1},{:.1},{:.2},{:.2},{:.0},{:.0},{:.0},{:.0},\"{}\"\n",
            self.tz.format(s.ts),
            s.temp.map(|t| format!("{:.1}", t)).unwrap_or_default(),
            s.power.map(|p| format!("{:.1}", p)).unwrap_or_default(),
            s.cpu,
            s.user,
            s.kernel,
            s.dpc,
            s.intr,
            s.freq,
            s.mem,
            s.disk,
            s.net,
            top
        );
        if let Some(f) = self.log_file.as_mut() {
            if f.write_all(line.as_bytes()).is_ok() {
                self.log_rows += 1;
                let _ = f.flush();
            }
        }
    }

    fn write_event(&mut self, e: &attrib::AttribEvent) {
        self.ensure_log();
        // 事件追加到独立的归因日志文件，便于检索
        let p = self.log_dir.join("归因事件.log");
        let mut buf = format!(
            "[{}] 温度 {:.1} → {:.1}（+{:.1}℃） 滞后{}s CPU {:.1}%（基线 {:.1}%） 频率 {:.0}→{:.0}MHz DPC {:.2}%\n  结论：{}\n  贡献者：{}\n",
            e.time,
            e.temp_from,
            e.temp_to,
            e.delta,
            e.lag,
            e.cpu_at,
            e.cpu_base,
            e.freq_base,
            e.freq_at,
            e.dpc_at,
            e.verdict,
            e.contributors
                .iter()
                .map(|c| format!("{}({:.1}%, Δ{:+.1})", c.0, c.1, c.2))
                .collect::<Vec<_>>()
                .join("  ")
        );
        if e.contributors.is_empty() {
            buf.push_str("  贡献者：无显著用户态进程\n");
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok();
        if let Some(f) = f.as_mut() {
            let _ = f.write_all(buf.as_bytes());
        }
    }
}

const LOG_README: &str = "本目录存放监控原始数据。\r\n\
每 10 秒追加一行 CSV：时间, 温度, 功耗, CPU%, 用户%, 内核%, DPC%, 中断%, 频率, 内存%, 磁盘, 网络, Top进程。\r\n\
归因事件.log：温度短时间内上升超过 2℃ 时记录的归因结论。\r\n\
可用 Excel 直接打开 CSV 做趋势分析。\r\n";

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn local_date(tz: &TzOffset) -> String {
    let days = (now_ts() + tz.0) / 86400;
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn autostart_enabled() -> bool {
    unsafe {
        let mut key: HKEY = ptr::null_mut();
        let r = RegOpenKeyExW(
            HKEY_CURRENT_USER as HKEY,
            collect::nt(r"Software\Microsoft\Windows\CurrentVersion\Run").as_ptr(),
            0,
            KEY_READ,
            &mut key,
        );
        if r != 0 {
            return false;
        }
        let mut ty: DWORD = 0;
        let mut size: DWORD = 0;
        let r2 = RegQueryValueExW(
            key,
            collect::nt("ThermalMonitor").as_ptr(),
            ptr::null(),
            &mut ty,
            ptr::null_mut(),
            &mut size,
        );
        RegCloseKey(key);
        r2 == 0
    }
}

fn set_autostart(on: bool) -> bool {
    unsafe {
        let mut key: HKEY = ptr::null_mut();
        let r = RegCreateKeyExW(
            HKEY_CURRENT_USER as HKEY,
            collect::nt(r"Software\Microsoft\Windows\CurrentVersion\Run").as_ptr(),
            0,
            ptr::null(),
            0,
            KEY_WRITE,
            ptr::null(),
            &mut key,
            ptr::null_mut(),
        );
        if r != 0 {
            return false;
        }
        let ok = if on {
            let mut v: Vec<u16> = exe_path().encode_utf16().chain(std::iter::once(0)).collect();
            RegSetValueExW(
                key,
                collect::nt("ThermalMonitor").as_ptr(),
                0,
                REG_SZ,
                v.as_mut_ptr() as *const u8,
                (v.len() * 2) as DWORD,
            ) == 0
        } else {
            RegDeleteValueW(key, collect::nt("ThermalMonitor").as_ptr()) == 0
        };
        RegCloseKey(key);
        ok
    }
}

fn open_dashboard() {
    unsafe {
        let url = collect::nt(&format!("http://127.0.0.1:{}/", PORT.load(Ordering::Relaxed)));
        ShellExecuteW(
            ptr::null_mut(),
            collect::nt("open").as_ptr(),
            url.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        );
    }
}

fn open_logs() {
    let dir = unsafe {
        match APP.get().and_then(|a| a.lock().ok()) {
            Some(a) => a.log_dir.to_string_lossy().to_string(),
            None => return,
        }
    };
    let _ = Command::new("explorer").arg(dir).spawn();
}

/// 绘制托盘图标：上半部温度数字，下半部 CPU% 小字，背景色随热度变化
unsafe fn make_icon(temp: Option<f32>, cpu: f32) -> HICON {
    let size = 32i32;
    let screen = GetDC(ptr::null_mut());
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, size, size);
    let old = SelectObject(dc, bmp);
    ReleaseDC(ptr::null_mut(), screen);

    let heat = temp.unwrap_or(0.0);
    let bg = if heat >= 82.0 {
        0x00_00_00_A8u32 // BGR: 深红
    } else if heat >= 70.0 {
        0x00_20_60_B8u32 // 橙
    } else {
        0x00_38_2A_18u32 // 深蓝灰
    };
    let rect = RECT { left: 0, top: 0, right: size, bottom: size };
    let brush = CreateSolidBrush(bg);
    FillRect(dc, &rect, brush);
    DeleteObject(brush);

    // 底部一条 CPU 占用指示条
    if cpu > 0.0 {
        let w = ((size as f32 - 4.0) * (cpu / 100.0).clamp(0.0, 1.0)) as i32;
        let br = RECT { left: 2, top: size - 4, right: 2 + w.max(1), bottom: size - 1 };
        let b = CreateSolidBrush(if cpu > 60.0 { 0x00_51_51_F8 } else { 0x00_51_A6_58 });
        FillRect(dc, &br, b);
        DeleteObject(b);
    }

    SetBkMode(dc, TRANSPARENT);
    let face = collect::nt("Segoe UI");
    let f1 = CreateFontW(
        -15, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, 0, face.as_ptr(),
    );
    let oldf = SelectObject(dc, f1);
    SetTextColor(dc, 0x00_FF_FF_FF);
    let mut r1 = RECT { left: 0, top: 1, right: size, bottom: 20 };
    let t = match temp {
        Some(v) => {
            let mut s = collect::nt(&format!("{:.0}", v));
            DrawTextW(dc, s.as_mut_ptr(), -1, &mut r1, 0x1 | 0x20 | 0x100);
            if s.len() > 1 {
                // 画右上角的小 ℃ 标记
                SetTextColor(dc, 0x00_A8_C8_FF);
                let f2 = CreateFontW(
                    -9, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, 0, face.as_ptr(),
                );
                SelectObject(dc, f2);
                let mut r2 = RECT { left: 20, top: 3, right: size, bottom: 14 };
                let _ = DrawTextW(dc, collect::nt("C").as_mut_ptr(), -1, &mut r2, 0x100);
                DeleteObject(f2);
            }
            s
        }
        None => {
            DrawTextW(dc, collect::nt("--").as_mut_ptr(), -1, &mut r1, 0x1 | 0x20 | 0x100);
            collect::nt("--")
        }
    };
    let _ = t;
    let _ = oldf;
    DeleteObject(f1);

    let f3 = CreateFontW(
        -10, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, 0, face.as_ptr(),
    );
    SelectObject(dc, f3);
    SetTextColor(dc, 0x00_D0_E8_FF);
    let mut r3 = RECT { left: 0, top: 16, right: size, bottom: 27 };
    DrawTextW(
        dc,
        collect::nt(&format!("{:.0}%", cpu)).as_mut_ptr(),
        -1,
        &mut r3,
        0x1 | 0x20 | 0x100,
    );
    DeleteObject(f3);

    // 全 0 的掩码位图 = 图标完全不透明
    let stride = ((size + 31) / 32) * 4;
    let mask_bits = vec![0u8; (stride * size) as usize];
    let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr() as *const std::ffi::c_void);
    let info = ICONINFO {
        f_icon: TRUE,
        x_hotspot: 0,
        y_hotspot: 0,
        hbm_mask: mask,
        hbm_color: bmp,
    };
    let icon = CreateIconIndirect(&info);
    let _ = old;
    // CreateIconIndirect 会复制位图，原图需自行释放，否则每秒重绘会持续泄漏 GDI 句柄
    DeleteObject(bmp);
    DeleteObject(mask);
    DeleteDC(dc);
    icon
}

/// 图标内容没有实质变化时不要重建，避免每秒创建 GDI 对象
fn icon_key(temp: Option<f32>, cpu: f32) -> (i32, i32) {
    (
        temp.map(|t| t.round() as i32).unwrap_or(-100),
        ((cpu.round() as i32) + 2) / 5 * 5,
    )
}

fn update_tray(hwnd: HWND, temp: Option<f32>, cpu: f32, ncores: usize, source: &str) {
    let key = icon_key(temp, cpu);
    let rebuild = {
        let slot = LAST_ICON_KEY.get_or_init(|| Mutex::new(None));
        let mut guard = match slot.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if *guard != Some(key) {
            *guard = Some(key);
            true
        } else {
            false
        }
    };
    unsafe {
        if rebuild {
            let icon = make_icon(temp, cpu);
            if icon.is_null() {
                return;
            }
            let prev = LAST_HICON.swap(icon, Ordering::SeqCst);
            if !prev.is_null() {
                DestroyIcon(prev);
            }
        }
        let icon = LAST_HICON.load(Ordering::SeqCst);
        if icon.is_null() {
            return;
        }
        let mut data: NOTIFYICONDATAW = mem::zeroed();
        data.cb_size = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
        data.hwnd = hwnd;
        data.uid = 1;
        data.uflags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.callback_message = TRAY_MSG;
        data.hicon = icon;

        let tip = format!(
            "温度 {} · CPU {:.0}% · {} 核\n{}\n双击打开看板，右键查看更多",
            temp.map(|t| format!("{:.0}℃", t)).unwrap_or_else(|| "未知".into()),
            cpu,
            ncores,
            source
        );
        let wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let n = wide.len().min(127);
        data.tip[..n].copy_from_slice(&wide[..n]);

        Shell_NotifyIconW(NIM_MODIFY, &mut data);
    }
}

fn on_tick(hwnd: HWND) {
    let (temp, cpu, ncores, source, need_event_log, new_event) = unsafe {
        match APP.get() {
            Some(a) => {
                let mut app = match a.lock() {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let before = app.attrib.events.len();
                let s = app.collector.sample();
                let ncores = app.collector.ncores;
                let src = app.collector.temp_source.label().to_string();
                app.hist.push_back(s.clone());
                if app.hist.len() > MAX_HIST {
                    app.hist.pop_front();
                }
                let App { attrib, hist, tz, .. } = &mut *app;
                attrib.evaluate(hist, tz);
                app.ticks += 1;
                if app.ticks % 10 == 0 {
                    app.write_row(&s);
                }
                let grew = app.attrib.events.len() > before;
                let ev = if grew {
                    Some(app.attrib.events[0].clone())
                } else {
                    None
                };
                if let Some(e) = ev.as_ref() {
                    app.write_event(e);
                }
                let temp = s.temp;
                let cpu = s.cpu;
                (temp, cpu, ncores, src, true, ev)
            }
            None => return,
        }
    };
    let _ = need_event_log;
    // 升温事件弹系统气泡提示
    if let Some(e) = new_event {
        unsafe {
            let mut d: NOTIFYICONDATAW = mem::zeroed();
            d.cb_size = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
            d.hwnd = hwnd;
            d.uid = 1;
            d.uflags = 0x10; // NIF_INFO
            d.timeout_or_version = 4;
            let title = collect::nt(&format!("温度 +{:.1}℃ → {}", e.delta, e.time));
            let body = collect::nt(&e.verdict);
            let nt = title.len().min(63);
            d.info_title[..nt].copy_from_slice(&title[..nt]);
            let nb = body.len().min(255);
            d.info[..nb].copy_from_slice(&body[..nb]);
            Shell_NotifyIconW(NIM_MODIFY, &mut d);
        }
    }
    update_tray(hwnd, temp, cpu, ncores, &source);
}

extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                if wparam == TIMER_ID {
                    on_tick(hwnd);
                }
                0
            }
            TRAY_MSG => {
                let event = (lparam as u32) & 0xffff;
                if event == 0x0205 || event == 0x007B {
                    // WM_RBUTTONUP / WM_CONTEXTMENU
                    let menu = CreatePopupMenu();
                    AppendMenuW(menu, MF_STRING, MENU_OPEN, collect::nt("打开监控看板").as_ptr());
                    AppendMenuW(menu, MF_STRING, MENU_LOGS, collect::nt("打开日志目录").as_ptr());
                    AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
                    let auto = if autostart_enabled() { "取消开机自启" } else { "设置开机自启" };
                    AppendMenuW(menu, MF_STRING, MENU_AUTOSTART, collect::nt(auto).as_ptr());
                    AppendMenuW(menu, MF_STRING, MENU_EXIT, collect::nt("退出").as_ptr());
                    let mut pt = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut pt);
                    SetForegroundWindow(hwnd);
                    let cmd = TrackPopupMenu(
                        menu,
                        TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
                        pt.x,
                        pt.y,
                        0,
                        hwnd,
                        ptr::null(),
                    ) as usize;
                    SetForegroundWindow(hwnd);
                    DestroyMenu(menu);
                    match cmd {
                        MENU_OPEN => open_dashboard(),
                        MENU_LOGS => open_logs(),
                        MENU_AUTOSTART => {
                            let _ = set_autostart(!autostart_enabled());
                        }
                        MENU_EXIT => {
                            PostQuitMessage(0);
                        }
                        _ => {}
                    }
                } else if event == 0x0203 {
                    // 双击
                    open_dashboard();
                }
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_loop() {
    unsafe {
        let hinst = GetModuleHandleW(ptr::null());
        let cls = collect::nt("ThermalMonitorClass");
        let mut wc: WNDCLASSW = mem::MaybeUninit::zeroed().assume_init();
        wc.lpfn_wnd_proc = wnd_proc;
        wc.h_instance = hinst;
        wc.lpsz_class_name = cls.as_ptr();
        wc.style = CS_HREDRAW | CS_VREDRAW;
        if RegisterClassW(&wc) == 0 && GetLastError() != 0x582 {
            // 类已存在时忽略
        }
        let hwnd = CreateWindowExW(
            0,
            cls.as_ptr(),
            collect::nt("ThermalMonitor").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null(),
        );
        if hwnd.is_null() {
            eprintln!("创建窗口失败: {}", GetLastError());
            return;
        }

        let icon = make_icon(None, 0.0);
        let mut data: NOTIFYICONDATAW = mem::zeroed();
        data.cb_size = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
        data.hwnd = hwnd;
        data.uid = 1;
        data.uflags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        data.callback_message = TRAY_MSG;
        data.hicon = icon;
        let tip: Vec<u16> = "温度监控启动中…".encode_utf16().chain(std::iter::once(0)).collect();
        data.tip[..tip.len()].copy_from_slice(&tip);
        if Shell_NotifyIconW(NIM_ADD, &mut data) != TRUE {
            eprintln!("托盘添加失败: {}", GetLastError());
        }
        LAST_HICON.store(icon, Ordering::SeqCst);

        SetTimer(hwnd, TIMER_ID, 1000, ptr::null());
        on_tick(hwnd);

        let mut msg: MSG = mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) != 0 {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }

        let mut d: NOTIFYICONDATAW = mem::zeroed();
        d.cb_size = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
        d.hwnd = hwnd;
        d.uid = 1;
        Shell_NotifyIconW(NIM_DELETE, &mut d);
        let prev = LAST_HICON.swap(ptr::null_mut(), Ordering::SeqCst);
        if !prev.is_null() {
            DestroyIcon(prev);
        }
        if let Some(a) = APP.get() {
            if let Ok(mut g) = a.lock() {
                g.collector.close();
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--diag") {
        collect::DIAG.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    if args
        .iter()
        .any(|a| a == "--debug" || a == "--console" || a == "--once" || a == "--diag")
    {
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
            AllocConsole();
        }
    }

    // 单实例保护
    unsafe {
        CreateMutexW(ptr::null(), FALSE, collect::nt("Global\\ThermalMonitorSingleton").as_ptr());
        if GetLastError() == 0xB7 {
            eprintln!("另一实例已在运行，托盘中查看即可。");
            return;
        }
    }

    // 命令行管理开机自启，便于脚本化部署
    if args.iter().any(|a| a == "--autostart") {
        let ok = set_autostart(true);
        println!("开机自启：{}", if ok { "已启用" } else { "设置失败" });
        return;
    }
    if args.iter().any(|a| a == "--no-autostart") {
        let ok = set_autostart(false);
        println!("开机自启：{}", if ok { "已取消" } else { "取消失败" });
        return;
    }

    let collector = match Collector::new() {
        Some(c) => c,
        None => {
            eprintln!("采集初始化失败：无法调用 NtQuerySystemInformation");
            return;
        }
    };

    // --once：采集一次并打印，用于自检
    if args.iter().any(|a| a == "--once") {
        let mut c = collector;
        let s = c.sample();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let s = c.sample();
        println!("核心数      : {}", s.cores.len());
        println!("CPU 占用    : {:.1}%  (用户 {:.1}% / 内核 {:.1}%)", s.cpu, s.user, s.kernel);
        println!("DPC/中断    : {:.2}% / {:.2}%", s.dpc, s.intr);
        println!("平均频率    : {:.0} MHz", s.freq);
        println!("温度        : {:?}", s.temp.map(|t| format!("{:.1} C", t)));
        println!("温度源      : {}", c.temp_source.label());
        println!("内存占用    : {:.0}%  磁盘 {:.0} KB/s  网络 {:.0} KB/s", s.mem, s.disk, s.net);
        println!("每核心      :");
        for (i, cc) in s.cores.iter().enumerate() {
            print!("  C{:<2} {:5.1}%  {:5} MHz  dpc {:.2}%", i, cc.usage, cc.freq as i64, cc.dpc);
            if (i + 1) % 4 == 0 {
                println!();
            }
        }
        if s.cores.len() % 4 != 0 {
            println!();
        }
        println!("Top 进程    :");
        for p in &s.procs {
            println!("  {:<38} {:6.2}%", p.0, p.1);
        }
        c.close();
        return;
    }

    let app = Arc::new(Mutex::new(App::new(collector)));

    let mut port: Option<u16> = None;
    for p in 8756..8766u16 {
        if let Some(got) = web::serve(&format!("127.0.0.1:{}", p), app.clone()) {
            port = Some(got);
            break;
        }
    }
    let port = match port {
        Some(p) => p,
        None => {
            eprintln!("无法启动本地监控服务");
            return;
        }
    };
    PORT.store(port, Ordering::SeqCst);
    let _ = APP.set(app);

    println!("监控服务已启动: http://127.0.0.1:{}", port);
    println!("托盘图标已创建，可在系统托盘查看温度与 CPU 占用。");

    run_loop();
}
