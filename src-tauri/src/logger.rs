use std::io::Write;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
static LOG_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

pub fn init(path: std::path::PathBuf) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("[zen-sync] cannot create log dir {}: {e}", parent.display());
                return;
            }
        }
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(f) => {
            let _ = LOG.set(Mutex::new(f));
            let _ = LOG_PATH.set(path);
        }
        Err(e) => eprintln!("[zen-sync] cannot open log {}: {e}", path.display()),
    }
}

pub fn path() -> Option<&'static std::path::Path> {
    LOG_PATH.get().map(|p| p.as_path())
}

pub fn log(msg: &str) {
    let ts = ts_now();
    eprintln!("{ts} {msg}");
    if let Some(f) = LOG.get() {
        if let Ok(mut f) = f.lock() {
            let _ = writeln!(f, "{ts} {msg}");
            let _ = f.flush();
        }
    }
}

fn ts_now() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sc = (s % 60) as u32;
    let mi = ((s / 60) % 60) as u32;
    let h = ((s / 3600) % 24) as u32;
    let mut days = (s / 86400) as u32;
    let mut y = 1970u32;
    loop {
        let dy = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = [31u32, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md { break; }
        days -= md;
        mo += 1;
    }
    let d = days + 1;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{sc:02}")
}

#[macro_export]
macro_rules! zslog {
    ($($arg:tt)*) => { $crate::logger::log(&format!($($arg)*)) };
}
