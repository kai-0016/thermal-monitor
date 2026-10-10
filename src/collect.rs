// 数据采集层：每核心负载/DPC/中断/频率、进程级 CPU、温度（多源自动探测）、内存/磁盘/网络。
use crate::ffi::*;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::slice;

pub static DIAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

macro_rules! diag {
    ($($a:tt)*) => {
        if DIAG.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!($($a)*);
        }
    };
}

pub fn nt(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn wchar_to_string(ptr: *const u16, max: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while len < max {
        if *ptr.add(len) == 0 {
            break;
        }
        len += 1;
    }
    String::from_utf16_lossy(slice::from_raw_parts(ptr, len))
}

#[derive(Clone, Copy, Default)]
pub struct CoreStat {
    pub usage: f32,
    pub freq: f32,
    pub dpc: f32,
    pub intr: f32,
}

#[derive(Clone)]
pub struct Sample {
    pub ts: i64,
    pub cpu: f32,
    pub user: f32,
    pub kernel: f32,
    pub dpc: f32,
    pub intr: f32,
    pub freq: f32,
    pub temp: Option<f32>,
    pub power: Option<f32>,
    pub mem: f32,
    pub disk: f32,
    pub net: f32,
    pub procs: Vec<(String, f32)>,
    pub cores: Vec<CoreStat>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TempSource {
    Hwinfo,
    ThermalZone,
    None,
}

impl TempSource {
    pub fn label(&self) -> &'static str {
        match self {
            TempSource::Hwinfo => "HWiNFO 共享内存（CPU 内核级传感器）",
            TempSource::ThermalZone => "ACPI 热区传感器（主板/CPU 周边，无需安装软件）",
            TempSource::None => "未找到可用温度传感器",
        }
    }
}

struct Pdh {
    open: FnPdhOpenQueryW,
    add: FnPdhAddEnglishCounterW,
    collect: FnPdhCollectQueryData,
    value: FnPdhGetFormattedCounterValue,
    array: FnPdhGetFormattedCounterArrayW,
}

struct Hwi {
    handle: HANDLE,
    base: *mut u8,
    n_readings: usize,
    off: usize,
}

/// Collector 内的裸指针（HWiNFO 内存映射、PDH 句柄）只在本进程主线程中使用，
/// 但 App 需要放进 Arc<Mutex<>> 里共享给 HTTP 线程读取，故显式标记为 Send/Sync。
/// 实际上 HTTP 线程只读取已采集好的 Sample 快照，从不触碰这些指针。
unsafe impl Send for Collector {}
unsafe impl Sync for Collector {}

pub struct Collector {
    nt_query: FnNtQuerySystemInformation,
    call_power: Option<FnCallNtPowerInformation>,
    pub ncores: usize,
    prev_core: Vec<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>,
    stride: usize,
    have_core: bool,
    prev_proc: HashMap<u64, u64>,
    pdh: Option<Pdh>,
    query: PDH_HQUERY,
    c_temp: PDH_HCOUNTER,
    c_disk: PDH_HCOUNTER,
    c_net: PDH_HCOUNTER,
    c_freq: PDH_HCOUNTER,
    hwi: Option<Hwi>,
    last_total: u64,
    max_mhz: u32,
    dpc_ema: Option<f32>,
    intr_ema: Option<f32>,
    pub temp_source: TempSource,
    pub hwinfo_hint: bool,
}

unsafe fn dyn_fn(module: *const u16, name: &str) -> *const c_void {
    let dll = LoadLibraryW(module);
    if dll.is_null() {
        return ptr::null();
    }
    let mut bytes = name.as_bytes().to_vec();
    bytes.push(0);
    GetProcAddress(dll, bytes.as_ptr())
}

/// PDH_MORE_DATA（按无符号比较，避免符号转换出错）
const PDH_MORE_DATA: u32 = 0x8000_07D2;

macro_rules! transmute_fn {
    ($p:expr, $t:ty) => {{
        let p = $p;
        if p.is_null() {
            None
        } else {
            Some(mem::transmute::<*const c_void, $t>(p))
        }
    }};
}

impl Collector {
    pub fn new() -> Option<Collector> {
        unsafe {
            let nt_query = transmute_fn!(
                dyn_fn(nt("ntdll.dll").as_ptr(), "NtQuerySystemInformation"),
                FnNtQuerySystemInformation
            )?;
            let call_power = transmute_fn!(
                dyn_fn(nt("powrprof.dll").as_ptr(), "CallNtPowerInformation"),
                FnCallNtPowerInformation
            );

            let ncores = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .max(1);

            diag!("[diag] ncores={} call_power={}", ncores, call_power.is_some());
            let mut c = Collector {
                nt_query,
                call_power,
                ncores,
                prev_core: Vec::new(),
                stride: 0,
                have_core: false,
                prev_proc: HashMap::new(),
                pdh: None,
                query: ptr::null_mut(),
                c_temp: ptr::null_mut(),
                c_disk: ptr::null_mut(),
                c_net: ptr::null_mut(),
                c_freq: ptr::null_mut(),
                hwi: None,
                last_total: 0,
                max_mhz: 0,
                dpc_ema: None,
                intr_ema: None,
                temp_source: TempSource::None,
                hwinfo_hint: false,
            };

            // PDH 初始化（用于热区温度、磁盘、网络）
            if let Some(open) = transmute_fn!(dyn_fn(nt("pdh.dll").as_ptr(), "PdhOpenQueryW"), FnPdhOpenQueryW)
            {
                let add = transmute_fn!(
                    dyn_fn(nt("pdh.dll").as_ptr(), "PdhAddEnglishCounterW"),
                    FnPdhAddEnglishCounterW
                );
                let collect = transmute_fn!(
                    dyn_fn(nt("pdh.dll").as_ptr(), "PdhCollectQueryData"),
                    FnPdhCollectQueryData
                );
                let value = transmute_fn!(
                    dyn_fn(nt("pdh.dll").as_ptr(), "PdhGetFormattedCounterValue"),
                    FnPdhGetFormattedCounterValue
                );
                let array = transmute_fn!(
                    dyn_fn(nt("pdh.dll").as_ptr(), "PdhGetFormattedCounterArrayW"),
                    FnPdhGetFormattedCounterArrayW
                );
                if let (Some(add), Some(collect), Some(value), Some(array)) = (add, collect, value, array)
                {
                    let mut q: PDH_HQUERY = ptr::null_mut();
                    if open(ptr::null(), 0, &mut q) == 0 {
                        let mut h: PDH_HCOUNTER = ptr::null_mut();
                        let r1 = add(q, nt(r"\Thermal Zone Information(*)\High Precision Temperature").as_ptr(), 0, &mut h);
                        diag!("[diag] add temp st=0x{:08X} c={:?}", r1, h);
                        if r1 == 0 {
                            c.c_temp = h;
                        }
                        let r2 = add(q, nt(r"\PhysicalDisk(_Total)\Disk Bytes/sec").as_ptr(), 0, &mut h);
                        diag!("[diag] add disk st=0x{:08X}", r2);
                        if r2 == 0 {
                            c.c_disk = h;
                        }
                        let r3 = add(q, nt(r"\Network Interface(*)\Bytes Total/sec").as_ptr(), 0, &mut h);
                        diag!("[diag] add net st=0x{:08X}", r3);
                        if r3 == 0 {
                            c.c_net = h;
                        }
                        // AMD 平台上 processor frequency 常只上报标称值，故用 % Performance × 标称频率换算实际频率
                        let r4 = add(q, nt(r"\Processor Information(*)\% Processor Performance").as_ptr(), 0, &mut h);
                        diag!("[diag] add freq st=0x{:08X}", r4);
                        if r4 == 0 {
                            c.c_freq = h;
                        }
                        c.query = q;
                        c.pdh = Some(Pdh { open, add, collect, value, array });
                        let _ = collect(q);
                    }
                }
            }

            // HWiNFO 共享内存（可选高精度数据源）
            if let Some(h) = Self::try_open_hwinfo() {
                c.hwi = Some(h);
                c.temp_source = TempSource::Hwinfo;
            } else {
                c.hwinfo_hint = Self::hwinfo_installed();
                if !c.c_temp.is_null() {
                    c.temp_source = TempSource::ThermalZone;
                }
            }

            // 预热一次采样，建立基线
            let _ = c.sample();
            Some(c)
        }
    }

    unsafe fn hwinfo_installed() -> bool {
        // 仅用于给用户提示：HWiNFO 是否已安装但未开启共享内存
        let mut key: HKEY = ptr::null_mut();
        let r = RegOpenKeyExW(
            HKEY_CURRENT_USER as HKEY,
            nt(r"Software\HWiNFO\Sensors").as_ptr(),
            0,
            KEY_READ,
            &mut key,
        );
        if r == 0 {
            RegCloseKey(key);
            true
        } else {
            false
        }
    }

    unsafe fn try_open_hwinfo() -> Option<Hwi> {
        let handle = OpenFileMappingW(FILE_MAP_READ, FALSE, nt("Global\\HWiNFO_SENSORS_SM2").as_ptr());
        if handle.is_null() {
            return None;
        }
        let base = MapViewOfFile(handle, SECTION_MAP_READ, 0, 0, 0) as *mut u8;
        if base.is_null() {
            CloseHandle(handle);
            return None;
        }
        let hdr = &*(base as *const HWI_SENSORS_SHARED_MEM2);
        if hdr.signature != 0x5369_5748 {
            UnmapViewOfFile(base as *const c_void);
            CloseHandle(handle);
            return None;
        }
        let version_ok = hdr.version >= 0x200;
        let size_ok = hdr.size_reading_section > 0
            && hdr.size_reading_section % HWI_READING_ELEMENT_SIZE as u32 == 0;
        if !(version_ok && size_ok) {
            UnmapViewOfFile(base as *const c_void);
            CloseHandle(handle);
            return None;
        }
        let n = hdr.size_reading_section as usize / HWI_READING_ELEMENT_SIZE;
        Some(Hwi { handle, base, n_readings: n, off: hdr.offset_reading_section as usize })
    }

    unsafe fn read_hwinfo(&self) -> Option<(Option<f32>, Option<f32>, Option<f32>)> {
        let h = self.hwi.as_ref()?;
        let mut temp: Option<f32> = None;
        let mut temp_score = 0u8;
        let mut power: Option<f32> = None;
        let mut fan: Option<f32> = None;
        let base = h.base.add(h.off);
        for i in 0..h.n_readings {
            let p = base.add(i * HWI_READING_ELEMENT_SIZE);
            let _sensor_id = ptr::read_unaligned(p as *const u32);
            let _reading_id = ptr::read_unaligned(p.add(4) as *const u32);
            let label_o = cstr_at(p, 8, 128);
            let label_u = cstr_at(p, 136, 128);
            let unit = cstr_at(p, 264, 16);
            let value = ptr::read_unaligned(p.add(280) as *const f64);
            let label = if !label_u.is_empty() { label_u } else { label_o };
            let l = label.to_lowercase();
            let u = unit.trim();
            if u == "\u{b0}C" || u == "C" || u == "\u{b0}c" {
                // 优先选择 CPU 控制温度，其次 CPU 封装，最后任意 CPU 相关
                let mut score = 0u8;
                if l.contains("tctl") || l.contains("tdie") {
                    score = 3;
                } else if l.contains("cpu package") {
                    score = 2;
                } else if l.contains("cpu") && !l.contains("gpu") {
                    score = 1;
                }
                if score > 0 && score >= temp_score && value > 5.0 && value < 125.0 {
                    temp_score = score;
                    temp = Some(value as f32);
                }
            } else if u.eq_ignore_ascii_case("W") {
                if (l.contains("package") || l.contains("ppt") || l.contains("soc")) && l.contains("power")
                    && value > 0.0 && value < 300.0
                {
                    power = Some(value as f32);
                }
            } else if u.eq_ignore_ascii_case("RPM") && l.contains("fan") {
                let v = value as f32;
                fan = Some(fan.map(|f: f32| f.max(v)).unwrap_or(v));
            }
        }
        if temp.is_some() || power.is_some() || fan.is_some() {
            Some((temp, power, fan))
        } else {
            None
        }
    }

    /// 读取某个多实例计数器的全部实例（两次调用：先取所需缓冲，再取数据）
    unsafe fn read_array(p: &Pdh, h: PDH_HCOUNTER) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        if h.is_null() {
            return out;
        }
        let mut size: DWORD = 0;
        let mut count: DWORD = 0;
        let st = (p.array)(h, PDH_FMT_DOUBLE, &mut size, &mut count, ptr::null_mut()) as u32;
        if st != PDH_MORE_DATA || size == 0 || count == 0 {
            diag!("[diag] array probe st=0x{:08X} size={} count={}", st, size, count);
            return out;
        }
        let unit = mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let cap = (size as usize / unit.max(1)).max(count as usize) + 8;
        let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = vec![
            PDH_FMT_COUNTERVALUE_ITEM_W {
                sz_name: ptr::null_mut(),
                fmt_value: PDH_FMT_COUNTERVALUE { c_status: 0, value: 0.0 },
            };
            cap
        ];
        let mut sz2: DWORD = (cap * unit) as DWORD;
        let mut c2: DWORD = 0;
        let st2 = (p.array)(h, PDH_FMT_DOUBLE, &mut sz2, &mut c2, buf.as_mut_ptr()) as u32;
        diag!("[diag] array pass2 st=0x{:08X} c2={}", st2, c2);
        if st2 != 0 {
            return out;
        }
        for i in 0..c2.min(cap as DWORD) as usize {
            let it = buf[i];
            if it.fmt_value.c_status != 0 {
                continue;
            }
            out.push((wchar_to_string(it.sz_name, 512), it.fmt_value.value));
        }
        out
    }

    unsafe fn read_pdh(&self) -> (Option<f32>, f32, f32, Option<Vec<f32>>) {
        let mut temp: Option<f32> = None;
        let mut disk = 0f32;
        let mut net = 0f32;
        let mut freqs: Option<Vec<f32>> = None;
        if let Some(p) = self.pdh.as_ref() {
            if !self.query.is_null() && (p.collect)(self.query) == 0 {
                let mut ctype: DWORD = 0;
                let mut val = PDH_FMT_COUNTERVALUE { c_status: 0, value: 0.0 };
                if !self.c_disk.is_null()
                    && (p.value)(self.c_disk, PDH_FMT_DOUBLE, &mut ctype, &mut val) == 0
                {
                    disk = (val.value / 1024.0) as f32;
                }
                if !self.c_net.is_null() {
                    for (n, v) in Self::read_array(p, self.c_net) {
                        let l = n.to_lowercase();
                        if !l.contains("loopback") && !l.contains("isatap") {
                            net += (v / 1024.0) as f32;
                        }
                    }
                }
                if !self.c_temp.is_null() {
                    let mut best: Option<f64> = None;
                    for (_n, v) in Self::read_array(p, self.c_temp) {
                        // High Precision Temperature 单位为 0.1 开尔文，取所有热区的最高值
                        if v > 1000.0 && v < 5000.0 {
                            best = Some(best.map(|b: f64| b.max(v)).unwrap_or(v));
                        }
                    }
                    temp = best.map(|v| (v / 10.0 - 273.15) as f32);
                }
                if !self.c_freq.is_null() {
                    let mut f = vec![0f32; self.ncores];
                    let mut hit = false;
                    let max = self.max_mhz.max(1) as f64;
                    for (n, v) in Self::read_array(p, self.c_freq) {
                        // 实例名形如 "0,0"，逗号后为逻辑处理器号
                        let tail = n.rsplit(',').next().unwrap_or("").trim().to_string();
                        if let Ok(idx) = tail.parse::<usize>() {
                            if idx < self.ncores && v > 0.0 {
                                f[idx] = (max * v / 100.0) as f32;
                                hit = true;
                            }
                        }
                    }
                    if hit {
                        freqs = Some(f);
                    }
                }
            }
        }
        (temp, disk, net, freqs)
    }

    unsafe fn cpu_cores(&mut self) -> Option<(Vec<CoreStat>, f32, f32, f32, f32, f32)> {
        // 该信息类要求缓冲区大小精确等于 ncpu × stride，先以长度 0 探询所需大小
        let mut need: ULONG = 0;
        let st0 = (self.nt_query)(
            SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION,
            ptr::null_mut(),
            0,
            &mut need,
        );
        if need == 0 {
            return None;
        }
        let mut buf: Vec<u8> = vec![0u8; need as usize];
        let mut ret: ULONG = 0;
        let st = (self.nt_query)(
            SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION,
            buf.as_mut_ptr() as *mut c_void,
            need,
            &mut ret,
        );
        diag!("[diag] core-perf probe st=0x{:08X} need={} st2=0x{:08X} ret={}", st0, need, st, ret);
        if st != 0 || ret == 0 {
            return None;
        }
        let stride = ret as usize / self.ncores;
        if stride < mem::size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>() || stride > 128 {
            return None;
        }
        self.stride = stride;

        let cur: Vec<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION> = (0..self.ncores)
            .map(|i| {
                ptr::read_unaligned(
                    buf.as_ptr().add(i * stride) as *const SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION
                )
            })
            .collect();

        // 频率（CallNtPowerInformation 作为兜底，PDH 每核频率优先）
        let mut freqs = vec![0u32; self.ncores];
        if let Some(cp) = self.call_power {
            let mut p: Vec<PROCESSOR_POWER_INFORMATION> = vec![
                PROCESSOR_POWER_INFORMATION {
                    number: 0, max_mhz: 0, current_mhz: 0, mhz_limit: 0,
                    max_idle_state: 0, current_idle_state: 0,
                };
                self.ncores
            ];
            let out_len = (self.ncores * mem::size_of::<PROCESSOR_POWER_INFORMATION>()) as ULONG;
            let ps = cp(PROCESSOR_INFORMATION_LEVEL, ptr::null_mut(), 0, p.as_mut_ptr() as *mut c_void, out_len);
            diag!("[diag] power st=0x{:08X} max={} cur={}", ps, p[0].max_mhz, p[0].current_mhz);
            if ps == 0 {
                self.max_mhz = p[0].max_mhz;
                for (i, item) in p.iter().enumerate().take(self.ncores) {
                    freqs[i] = item.current_mhz;
                }
            }
        }

        let mut cores = Vec::with_capacity(self.ncores);
        let (mut s_busy, mut s_total, mut s_user, mut s_kernel, mut s_dpc, mut s_intr) =
            (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);

        for i in 0..self.ncores {
            let c = cur[i];
            let (mut usage, mut dpc, mut intr) = (0f32, 0f32, 0f32);
            if self.have_core && self.prev_core.len() == self.ncores {
                let p = self.prev_core[i];
                // 注意 KernelTime 已包含 IdleTime：busy = kernel + user - idle
                let busy = (c.kernel_time.wrapping_add(c.user_time).wrapping_sub(c.idle_time))
                    .wrapping_sub(p.kernel_time.wrapping_add(p.user_time).wrapping_sub(p.idle_time))
                    as f64;
                let total = c
                    .kernel_time
                    .wrapping_add(c.user_time)
                    .wrapping_sub(p.kernel_time.wrapping_add(p.user_time))
                    as f64;
                let user = c.user_time.wrapping_sub(p.user_time) as f64;
                let kernel = (c.kernel_time.wrapping_sub(c.idle_time))
                    .wrapping_sub(p.kernel_time.wrapping_sub(p.idle_time))
                    as f64;
                let dpc_d = c.dpc_time.wrapping_sub(p.dpc_time) as f64;
                let intr_d = c.interrupt_time.wrapping_sub(p.interrupt_time) as f64;
                if total > 0.0 {
                    usage = ((busy / total) * 100.0) as f32;
                    dpc = ((dpc_d / total) * 100.0) as f32;
                    intr = ((intr_d / total) * 100.0) as f32;
                }
                s_busy += busy;
                s_total += total;
                s_user += user;
                s_kernel += kernel;
                s_dpc += dpc_d;
                s_intr += intr_d;
            }
            cores.push(CoreStat {
                usage: if usage < 0.0 { 0.0 } else { usage }.min(100.0),
                freq: freqs[i] as f32,
                dpc: dpc.max(0.0),
                intr: intr.max(0.0),
            });
        }

        self.prev_core = cur;
        let first = !self.have_core;
        self.have_core = true;

        let (cpu, user, kernel, dpc_t, intr_t) = if s_total > 0.0 {
            (
                ((s_busy / s_total) * 100.0) as f32,
                ((s_user / s_total) * 100.0) as f32,
                ((s_kernel / s_total) * 100.0) as f32,
                ((s_dpc / s_total) * 100.0) as f32,
                ((s_intr / s_total) * 100.0) as f32,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0)
        };
        if first {
            None
        } else {
            Some((cores, cpu, user, kernel, dpc_t, intr_t))
        }
    }

    /// 进程级 CPU%：一次 NtQuerySystemInformation 拿到全部进程，开销极低。
    unsafe fn processes(&mut self, total_delta: f64) -> Vec<(String, f32)> {
        let mut buf: Vec<u8> = Vec::with_capacity(1 << 19);
        let mut ret: ULONG = 0;
        let (mut p, mut st) = (ptr::null_mut::<SYSTEM_PROCESS_INFORMATION>(), -1i32);
        for _ in 0..4 {
            let cap = buf.capacity() as ULONG;
            st = (self.nt_query)(
                SYSTEM_PROCESS_INFORMATION,
                buf.as_mut_ptr() as *mut c_void,
                cap,
                &mut ret,
            );
            if st == 0 {
                p = buf.as_ptr() as *mut SYSTEM_PROCESS_INFORMATION;
                break;
            }
            if ret == 0 {
                break;
            }
            let want = (ret as usize).max(buf.capacity() * 2);
            buf.reserve(want);
        }
        if st != 0 || p.is_null() {
            return Vec::new();
        }

        let mut cur: HashMap<u64, u64> = HashMap::with_capacity(256);
        let mut names: HashMap<u64, String> = HashMap::with_capacity(256);
        let mut node = p;
        let mut guard = 0usize;
        loop {
            let off = (*node).next_entry_offset;
            let times = (*node).kernel_time.wrapping_add((*node).user_time);
            let pid = (*node).process_id;
            let nthreads = (*node).number_of_threads;
            // 布局自检：出现异常值说明结构体偏移不对，宁可放弃也不输出脏数据
            if nthreads > 0x10000 || pid > 0x10_0000 {
                self.prev_proc.clear();
                return Vec::new();
            }
            let name = if !(*node).name_buffer.is_null() {
                wchar_to_string((*node).name_buffer, ((*node).name_length as usize) / 2)
            } else {
                String::new()
            };
            cur.insert(pid, times);
            names.insert(pid, name);
            if off == 0 || guard > 4096 {
                break;
            }
            node = (node as *mut u8).add(off as usize) as *mut SYSTEM_PROCESS_INFORMATION;
            guard += 1;
        }

        let mut out: Vec<(String, f32)> = Vec::new();
        if total_delta > 0.0 {
            for (pid, t) in cur.iter() {
                if *pid == 0 {
                    continue;
                }
                if let Some(prev) = self.prev_proc.get(pid) {
                    let d = t.wrapping_sub(*prev) as f64;
                    let pct = ((d / total_delta) * 100.0) as f32;
                    if pct > 0.05 {
                        let n = names.get(pid).cloned().unwrap_or_else(|| format!("pid:{}", pid));
                        out.push((n, pct.min(100.0 * self.ncores as f32)));
                    }
                }
            }
        }
        self.prev_proc = cur;

        // 按进程名聚合（现代应用普遍多进程：Chrome/Edge/微信/各类 Electron 应用）
        let mut agg: HashMap<String, f32> = HashMap::with_capacity(32);
        for (n, v) in out {
            *agg.entry(n).or_insert(0.0) += v;
        }
        let mut res: Vec<(String, f32)> = agg.into_iter().collect();
        res.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        res.truncate(14);
        res
    }

    /// 对 DPC/中断这类抖动较大的读数做指数滑动平均，避免看板数字乱跳
    fn ema(prev: &mut Option<f32>, v: f32) -> f32 {
        let v = v.max(0.0);
        let out = match prev {
            Some(p) => *p * 0.65 + v * 0.35,
            None => v,
        };
        *prev = Some(out);
        out
    }

    pub fn sample(&mut self) -> Sample {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let core = unsafe { self.cpu_cores() };
        let (mut cores, cpu, user, kernel, dpc, intr) =
            core.unwrap_or_else(|| (vec![CoreStat::default(); self.ncores], 0.0, 0.0, 0.0, 0.0, 0.0));

        // 用系统总 CPU 时间增量作为进程百分比分母（已隐含核心数）
        let mut total_delta = 0f64;
        let mut idle = 0u64;
        let mut krnl = 0u64;
        let mut usr = 0u64;
        unsafe {
            GetSystemTimes(&mut idle, &mut krnl, &mut usr);
        }
        let now_total = krnl.wrapping_add(usr);
        if self.last_total > 0 {
            total_delta = now_total.wrapping_sub(self.last_total) as f64;
        }
        self.last_total = now_total;

        let procs = unsafe { self.processes(total_delta) };

        let (t_pdh, disk, net, core_freqs) = unsafe { self.read_pdh() };
        if let Some(cf) = core_freqs {
            for (i, c) in cores.iter_mut().enumerate() {
                if i < cf.len() && cf[i] > 100.0 {
                    c.freq = cf[i];
                }
            }
        }
        let freq = cores.iter().map(|c| c.freq).sum::<f32>() / self.ncores.max(1) as f32;
        let (hw_temp, power) = match unsafe { self.read_hwinfo() } {
            Some((t, p, _f)) => (t, p),
            None => (None, None),
        };
        let temp = hw_temp.or(t_pdh);
        self.temp_source = if hw_temp.is_some() {
            TempSource::Hwinfo
        } else if t_pdh.is_some() {
            TempSource::ThermalZone
        } else {
            TempSource::None
        };

        let mut mem = MEMORYSTATUSEX {
            dw_length: mem::size_of::<MEMORYSTATUSEX>() as DWORD,
            dw_memory_load: 0,
            ull_total_phys: 0,
            ull_avail_phys: 0,
            ull_total_page_file: 0,
            ull_avail_page_file: 0,
            ull_total_virtual: 0,
            ull_avail_virtual: 0,
            ull_avail_extended_virtual: 0,
        };
        let mem_pct = unsafe { GlobalMemoryStatusEx(&mut mem) };
        let memp = if mem_pct != 0 { mem.dw_memory_load as f32 } else { 0.0 };

        Sample {
            ts,
            cpu: cpu.clamp(0.0, 100.0),
            user,
            kernel,
            dpc: Self::ema(&mut self.dpc_ema, dpc),
            intr: Self::ema(&mut self.intr_ema, intr),
            freq,
            temp,
            power,
            mem: memp,
            disk,
            net,
            procs,
            cores,
        }
    }

    pub fn close(&mut self) {
        if let Some(h) = self.hwi.take() {
            unsafe {
                UnmapViewOfFile(h.base as *const c_void);
                CloseHandle(h.handle);
            }
        }
    }
}

unsafe fn cstr_at(base: *mut u8, offset: usize, max: usize) -> String {
    let p = base.add(offset);
    let mut len = 0usize;
    while len < max {
        if *p.add(len) == 0 {
            break;
        }
        len += 1;
    }
    if len == 0 {
        return String::new();
    }
    String::from_utf8_lossy(slice::from_raw_parts(p, len)).to_string()
}
