# 温度 · 占用 · 升温归因监控器

一个为 Windows 写的**极轻量**后台监控工具：记录 CPU 温度、每核心负载/真实频率、进程级占用，并**自动回答"刚才温度为什么升高"**。

* 单文件 exe，**零第三方依赖、零安装**（Rust + 手写 Win32 API）
* 常驻内存约 **18 MB**，自身 CPU 占用约 **0.05%**（占比 < 0.1%）
* 数据全在本地，**不联网、不上传任何内容**

---

## 快速开始

1. 双击 `thermal-monitor.exe` —— 程序静默进入系统托盘，**不弹窗**
2. 托盘上直接显示：**上行 = 当前温度，下行 = CPU 占用**，鼠标悬停看详情
3. **双击托盘**打开网页看板；**右键**菜单：
   - 打开监控看板
   - 打开日志目录
   - 设置/取消开机自启
   - 退出

命令行可选参数：

| 参数 | 作用 |
|---|---|
| `--once` | 采集一次并打印到控制台，然后退出（自检用） |
| `--diag` | 打印数据源诊断信息（排错用） |
| `--autostart` / `--no-autostart` | 命令行启用/取消开机自启 |
| `--console` | 保留控制台窗口运行 |

---

## 看板能看到什么

浏览器打开 `http://127.0.0.1:8756`（托盘菜单也可打开）：

* **KPI**：温度、CPU 占用、平均频率、内核 DPC/中断、内存、磁盘、网络、功耗
* **温度趋势图**（近 10 分钟）+ **CPU/频率图**
* **每核心**：负载百分比、实时频率
* **进程占用排行**（同名多进程已合并）
* **升温归因事件**：温度短时上升超过 2℃ 时自动生成一条结论，例如：

  > `[11:36:27] +2.2℃（77.5 → 79.7）负载滞后 30s，CPU 100%（基线 65.3%），可信度 78%`
  > 结论：pwsh.exe 占用飙到 71.4%，是这次升温的主因
  > 贡献者：pwsh.exe 71.4%（Δ+71.4） CodeBuddy CN.exe 14.8%（Δ+3.7）…

数据每秒刷新，最多保留 1 小时曲线。

---

## 归因是怎么做的

温度相对负载有**热惯性**，所以不能拿同一秒的进程占用去解释温度。流程是：

1. 滑动窗口每 5 秒评估：当前温度比 40 秒前上升 ≥ 2℃ 则判定为升温事件
2. **滞后对齐**：在 0～45 秒范围内搜索使「负载序列 vs 温度序列」皮尔逊相关最大的滞后值
3. 对齐到该时刻，取当时占用最高的若干进程，并对比其更早的历史基线算出增量 Δ
4. 排除用户态进程后仍有异常时，会提示**内核 DPC/中断**或**高频睿频**——这是"没开软件却发热"的典型成因

---

## 数据源说明

### 温度（自动探测，按优先级）

| 优先级 | 数据源 | 说明 |
|---|---|---|
| 1 | **HWiNFO 共享内存** | CPU 内核级 Tctl/Tdie + 实时功耗。需另外安装 HWiNFO 并在 Settings 勾选 **Shared Memory Support**，检测到后自动切换 |
| 2 | **ACPI 热区传感器**（默认，零依赖） | 通过 PDH 读 `Thermal Zone Information`，无需安装任何软件。绝对值通常比 CPU 裸片高几度，**但趋势与归因结论准确** |

### 为什么没有"每核心温度"

这是硬件限制：**AMD Zen 全系不提供 per-core 温度传感器**，只上报封装级 Tctl/Tdie；Intel 才有每核 DTS。因此程序改为展示「封装温度曲线 + **每核心负载与真实频率**」。

> 补充：本机 AMD 平台上，Windows 的标准接口（`CallNtPowerInformation`、PDH `Processor Frequency`）只上报标称频率 3801MHz。程序改用 `% Processor Performance × 标称频率` 换算出真实频率（实测 3873～4636MHz 变动正常）。

### 每核心负载 / DPC / 中断

来自 `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`，一次系统调用取全核心数据。
注意其 `KernelTime` 包含 `IdleTime`，程序按 `busy = kernel + user - idle` 计算。

### 进程级占用

一次 `NtQuerySystemInformation(SystemProcessInformation)` 取全部进程，与 `GetSystemTimes` 的时间增量作比。
多个同名进程（Chrome、Edge、各类 Electron 应用）已按进程名聚合。

---

## 日志文件

位于 `%LOCALAPPDATA%\ThermalMonitor\logs`：

* `yyyy-MM-dd.csv`：每 10 秒一行（时间、温度、功耗、CPU%、用户%/内核%、DPC%、中断%、频率、内存、磁盘、网络、Top3 进程），可用 Excel 打开做趋势分析
* `归因事件.log`：每次判定升温的结论文本

---

## 编译

需要 Rust 工具链（本项目在 Rust 1.96 上开发编译）：

```bash
cargo build --release
```

产物：`target/release/thermal-monitor.exe`（无外部依赖，可单独拷贝）。
Windows 链接 `advapi32 / gdi32 / shell32 / user32 / ole32`，见 `build.rs`。

## 代码结构

| 文件 | 职责 |
|---|---|
| `src/ffi.rs` | 手写 Win32 API 声明与结构体 |
| `src/collect.rs` | 采集：每核负载/DPC/中断/频率、进程、温度多源、内存磁盘网络 |
| `src/attrib.rs` | 归因引擎：滞后对齐、相关性、贡献者识别、结论生成 |
| `src/web.rs` | 本地 HTTP 服务与 JSON 接口 |
| `src/dashboard.rs` | 单文件网页看板（离线，无 CDN） |
| `src/main.rs` | 托盘图标渲染、消息循环、日志、开机自启 |
