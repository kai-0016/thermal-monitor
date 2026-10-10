// 升温归因引擎：温度对负载有热惯性，先把负载与温度做滞后对齐，再找出真正的贡献者。
use crate::collect::Sample;
use crate::ffi::*;
use std::collections::VecDeque;
use std::mem;

pub struct TzOffset(pub i64);

impl TzOffset {
    pub fn detect() -> TzOffset {
        unsafe {
            #[allow(invalid_value)]
            let mut info: TIME_ZONE_INFORMATION = mem::MaybeUninit::zeroed().assume_init();
            GetTimeZoneInformation(&mut info);
            // UTC = 本地时间 + Bias（分钟）
            TzOffset(-(info.bias as i64) * 60)
        }
    }
    pub fn format(&self, ts: i64) -> String {
        let local = ts + self.0;
        let secs = ((local % 86400) + 86400) % 86400;
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
}

#[derive(Clone)]
pub struct AttribEvent {
    pub id: u64,
    pub time: String,
    pub ts: i64,
    pub temp_from: f32,
    pub temp_to: f32,
    pub delta: f32,
    pub lag: usize,
    pub cpu_at: f32,
    pub cpu_base: f32,
    pub freq_at: f32,
    pub freq_base: f32,
    pub dpc_at: f32,
    pub contributors: Vec<(String, f32, f32)>, // 名称, 当时占用%, 相对基线增量
    pub verdict: String,
    pub confidence: f32,
}

pub struct AttribEngine {
    pub events: Vec<AttribEvent>,
    next_id: u64,
    last_emit: i64,
}

const RECENT_WINDOW: usize = 6; // 当前温度取最近 6 个采样均值，抑制抖动
const TEMP_RISE: f32 = 2.0; // 判定为升温事件的阈值（℃）
const LOOK_BACK: usize = 45;

impl AttribEngine {
    pub fn new() -> AttribEngine {
        AttribEngine { events: Vec::new(), next_id: 1, last_emit: 0 }
    }

    fn avg_temp(hist: &VecDeque<Sample>, end: usize, n: usize) -> Option<f32> {
        if hist.len() < end + n {
            return None;
        }
        let last = hist.len() - 1;
        let mut sum = 0f32;
        let mut cnt = 0usize;
        for k in 0..n {
            let idx = last - end - k;
            if let Some(t) = hist[idx].temp {
                sum += t;
                cnt += 1;
                let _ = sum;
            }
        }
        if cnt == 0 { None } else { Some(sum / cnt as f32) }
    }

    /// 皮尔逊相关：温度序列 vs 滞后 lag 秒的 CPU 序列
    fn corr_cpu_temp(hist: &VecDeque<Sample>, win: usize, lag: usize) -> f32 {
        let last = hist.len() - 1;
        if last < win + lag {
            return 0.0;
        }
        let mut xs: Vec<f32> = Vec::with_capacity(win);
        let mut ys: Vec<f32> = Vec::with_capacity(win);
        for i in 0..win {
            let dst = last - i;
            let src = last - i - lag;
            if let Some(t) = hist[dst].temp {
                xs.push(hist[src].cpu);
                ys.push(t);
            }
        }
        if xs.len() < 20 {
            return 0.0;
        }
        let n = xs.len() as f32;
        let mx = xs.iter().sum::<f32>() / n;
        let my = ys.iter().sum::<f32>() / n;
        let mut num = 0f32;
        let mut dx = 0f32;
        let mut dy = 0f32;
        for i in 0..xs.len() {
            let a = xs[i] - mx;
            let b = ys[i] - my;
            num += a * b;
            dx += a * a;
            dy += b * b;
        }
        if dx <= 0.0 || dy <= 0.0 {
            0.0
        } else {
            (num / (dx.sqrt() * dy.sqrt())).clamp(-1.0, 1.0)
        }
    }

    /// 某个进程在窗口内的平均占用
    fn proc_avg(hist: &VecDeque<Sample>, end: usize, n: usize, name: &str) -> f32 {
        let last = hist.len() - 1;
        let mut sum = 0f32;
        let mut cnt = 0usize;
        for k in 0..n {
            let idx = last - end - k;
            for p in &hist[idx].procs {
                if p.0 == name {
                    sum += p.1;
                    cnt += 1;
                    break;
                }
            }
        }
        if cnt == 0 { 0.0 } else { sum / cnt as f32 }
    }

    pub fn evaluate(&mut self, hist: &VecDeque<Sample>, tz: &TzOffset) {
        if hist.len() < 90 {
            return;
        }
        let now_temp = match Self::avg_temp(hist, 0, RECENT_WINDOW) {
            Some(t) => t,
            None => return,
        };
        let ref_temp = match Self::avg_temp(hist, 40, RECENT_WINDOW) {
            Some(t) => t,
            None => return,
        };
        let rise = now_temp - ref_temp;
        if rise < TEMP_RISE {
            return;
        }
        let now_ts = hist[hist.len() - 1].ts;
        if now_ts - self.last_emit < 50 {
            return;
        }
        self.last_emit = now_ts;

        // 搜索最佳滞后：0..LOOK_BACK 秒，步长 3
        let mut best_lag = 0usize;
        let mut best_c = f32::NEG_INFINITY;
        let win = 90.min(hist.len() - 1);
        let mut lag = 0usize;
        while lag <= LOOK_BACK {
            let c = Self::corr_cpu_temp(hist, win, lag);
            if c > best_c {
                best_c = c;
                best_lag = lag;
            }
            lag += 3;
        }

        let last = hist.len() - 1;
        let at = &hist[last.saturating_sub(best_lag)];
        let cpu_at = at.cpu;
        let freq_at = at.freq;
        let dpc_at = at.dpc + at.intr;

        // 基线：更早的历史（30~60 秒前）
        let mut cpu_base = 0f32;
        let mut freq_base = 0f32;
        let mut cnt = 0usize;
        if last > 70 {
            // 取更早的历史窗口作为对比基线
            for i in (last - 40)..=(last - 25) {
                cpu_base += hist[i].cpu;
                freq_base += hist[i].freq;
                cnt += 1;
            }
        }
        if cnt > 0 {
            cpu_base /= cnt as f32;
            freq_base /= cnt as f32;
        } else {
            cpu_base = 0.0;
            freq_base = freq_at;
        }

        // 贡献者：滞后对齐时刻的高占用进程，并算出相对更早基线的增量
        let mut contributors: Vec<(String, f32, f32)> = Vec::new();
        for p in &at.procs {
            let avg = Self::proc_avg(hist, 60, 25, &p.0);
            let delta = p.1 - avg;
            if p.1 >= 1.0 || delta >= 1.0 {
                contributors.push((p.0.clone(), p.1, delta));
            }
        }
        contributors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        contributors.truncate(4);

        let verdict = if let Some(top) = contributors.first() {
            if top.1 >= 30.0 || top.2 >= 15.0 {
                format!("{} 占用飙到 {:.1}%（均值基线 {:.1}%），是这次升温的主因", top.0, top.1, (top.1 - top.2).max(0.0))
            } else if top.2 >= 3.0 {
                format!("{} 占用从 {:.1}% 涨到 {:.1}%，主要推高了温度", top.0, (top.1 - top.2).max(0.0), top.1)
            } else {
                "负载分布较平均，属于多进程共同推高，无明显单一元凶".to_string()
            }
        } else if dpc_at > 1.0 {
            format!("用户态进程占用很低，但内核中断/DPC 占 {:.2}% —— 疑似驱动在后台忙，这就是“没开软件也发热”的典型特征", dpc_at)
        } else if cpu_at < 20.0 && freq_at > freq_base + 200.0 {
            format!("占用不高但频率从 {:.0}MHz 拉到 {:.0}MHz，高频睿频本身在推高温升", freq_base, freq_at)
        } else {
            "未定位到明确的用户态进程，可能是散热条件变化（积灰/风道/环境温度）或传感器漂移".to_string()
        };

        let ev = AttribEvent {
            id: self.next_id,
            time: tz.format(now_ts),
            ts: now_ts,
            temp_from: ref_temp,
            temp_to: now_temp,
            delta: rise,
            lag: best_lag,
            cpu_at,
            cpu_base,
            freq_at,
            freq_base,
            dpc_at,
            contributors,
            verdict,
            confidence: best_c.max(0.0),
        };
        self.next_id += 1;
        self.events.insert(0, ev);
        if self.events.len() > 60 {
            self.events.truncate(60);
        }
    }
}
