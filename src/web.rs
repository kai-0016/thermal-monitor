// 极简本地 HTTP 服务 + 单文件网页看板。全部离线，不依赖任何外部 CDN。
use crate::collect::Sample;
use crate::App;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn f0(v: f32) -> String {
    format!("{:.0}", v)
}
fn f1(v: f32) -> String {
    format!("{:.1}", v)
}
fn f2(v: f32) -> String {
    format!("{:.2}", v)
}

fn json_sample(s: &Sample, tz: &crate::attrib::TzOffset, source: &str) -> String {
    let mut procs = String::new();
    for (n, v) in &s.procs {
        if !procs.is_empty() {
            procs.push(',');
        }
        procs.push_str(&format!("[\"{}\",{}]", esc(n), f2(*v)));
    }
    let mut cores = String::new();
    for c in &s.cores {
        if !cores.is_empty() {
            cores.push(',');
        }
        cores.push_str(&format!(
            "[{},{},{},{}]",
            f1(c.usage),
            f0(c.freq),
            f2(c.dpc),
            f2(c.intr)
        ));
    }
    format!(
        concat!(
            "{{\"ts\":{},\"time\":\"{}\",\"cpu\":{},\"user\":{},\"kernel\":{},\"dpc\":{},\"intr\":{},",
            "\"freq\":{},\"temp\":{},\"power\":{},\"mem\":{},\"disk\":{},\"net\":{},",
            "\"source\":\"{}\",\"cores\":[{}],\"procs\":[{}]}}"
        ),
        s.ts,
        tz.format(s.ts),
        f1(s.cpu),
        f1(s.user),
        f1(s.kernel),
        f2(s.dpc),
        f2(s.intr),
        f0(s.freq),
        s.temp.map(f1).unwrap_or_else(|| "null".into()),
        s.power.map(f1).unwrap_or_else(|| "null".into()),
        f0(s.mem),
        f0(s.disk),
        f0(s.net),
        esc(source),
        cores,
        procs
    )
}

fn json_events(app: &App) -> String {
    let mut out = String::new();
    for e in app.attrib.events.iter().take(12) {
        if !out.is_empty() {
            out.push(',');
        }
        let mut cs = String::new();
        for c in &e.contributors {
            if !cs.is_empty() {
                cs.push(',');
            }
            cs.push_str(&format!("[\"{}\",{},{}]", esc(&c.0), f1(c.1), f1(c.2)));
        }
        out.push_str(&format!(
            concat!(
                "{{\"id\":{},\"time\":\"{}\",\"from\":{},\"to\":{},\"delta\":{},\"lag\":{},",
                "\"cpu\":{},\"cpuBase\":{},\"freq\":{},\"freqBase\":{},\"dpc\":{},\"conf\":{},",
                "\"verdict\":\"{}\",\"c\":[{}]}}"
            ),
            e.id,
            e.time,
            f1(e.temp_from),
            f1(e.temp_to),
            f1(e.delta),
            e.lag,
            f1(e.cpu_at),
            f1(e.cpu_base),
            f0(e.freq_at),
            f0(e.freq_base),
            f2(e.dpc_at),
            f2(e.confidence),
            esc(&e.verdict),
            cs
        ));
    }
    out
}

fn json_series(app: &App, n: usize) -> String {
    let h = &app.hist;
    let start = h.len().saturating_sub(n);
    let (mut t, mut cpu, mut temp, mut freq, mut dpc, mut mem, mut power) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    for i in start..h.len() {
        let s = &h[i];
        if !t.is_empty() {
            t.push(',');
            cpu.push(',');
            temp.push(',');
            freq.push(',');
            dpc.push(',');
            mem.push(',');
            power.push(',');
        }
        t.push_str(&s.ts.to_string());
        cpu.push_str(&f1(s.cpu));
        temp.push_str(&s.temp.map(f1).unwrap_or_else(|| "null".into()));
        freq.push_str(&f0(s.freq));
        dpc.push_str(&f2(s.dpc + s.intr));
        mem.push_str(&f0(s.mem));
        power.push_str(&s.power.map(f1).unwrap_or_else(|| "null".into()));
    }
    format!(
        "{{\"t\":[{}],\"cpu\":[{}],\"temp\":[{}],\"freq\":[{}],\"dpc\":[{}],\"mem\":[{}],\"power\":[{}]}}",
        t, cpu, temp, freq, dpc, mem, power
    )
}

fn json_all(app: &App, n: usize) -> String {
    let cur = app
        .hist
        .back()
        .map(|s| json_sample(s, &app.tz, app.collector.temp_source.label()))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"cur\":{},\"series\":{},\"events\":[{}],\"uptime\":{},\"ncores\":{},\"log\":\"{}\"}}",
        cur,
        json_series(app, n),
        json_events(app),
        app.started_elapsed_secs(),
        app.collector_ncores(),
        esc(&app.log_status())
    )
}

fn send(stream: &mut TcpStream, body: &[u8], ctype: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        ctype,
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn handle(mut stream: TcpStream, app: Arc<Mutex<App>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
    let mut buf = [0u8; 4096];
    let mut req = Vec::new();
    let first_line = loop {
        match stream.read(&mut buf) {
            Ok(0) => break String::new(),
            Ok(n) => {
                req.extend_from_slice(&buf[..n]);
                if req.windows(4).any(|w| w == b"\r\n\r\n") || req.len() > 8192 {
                    break String::from_utf8_lossy(&req).to_string();
                }
            }
            Err(_) => break String::new(),
        }
    };
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();

    if path.starts_with("/api/all") || path.starts_with("/api/state") {
        let n: usize = path
            .find('n')
            .and_then(|i| path[i..].split('=').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(600)
            .clamp(60, 3600);
        let body = match app.lock() {
            Ok(a) => json_all(&a, n),
            Err(_) => "{}".into(),
        };
        send(&mut stream, body.as_bytes(), "application/json; charset=utf-8");
        return;
    }
    if path.starts_with("/favicon") {
        send(&mut stream, b"", "image/x-icon");
        return;
    }
    if path.starts_with("/api/exit") {
        let body = b"{\"ok\":true}";
        send(&mut stream, body, "application/json; charset=utf-8");
        std::process::exit(0);
    }
    let html = crate::dashboard::html();
    send(&mut stream, html.as_bytes(), "text/html; charset=utf-8");
}

pub fn serve(addr: &str, app: Arc<Mutex<App>>) -> Option<u16> {
    let listener = match std::net::TcpListener::bind(addr) {
        Ok(l) => l,
        Err(_) => return None,
    };
    let port = listener.local_addr().ok().map(|a| a.port());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let a = app.clone();
                    std::thread::spawn(move || handle(s, a));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    });
    port
}
