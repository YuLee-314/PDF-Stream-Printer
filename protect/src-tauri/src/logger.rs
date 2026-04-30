use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub time: String,
    pub file: String,
    pub pages: u32,
    pub status: String,
    pub duration: String,
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("big_pdf_printer")
}

fn history_path() -> PathBuf {
    temp_dir().join("history.json")
}

fn load() -> Vec<LogRecord> {
    let p = history_path();
    if !p.is_file() {
        return vec![];
    }
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(records: &[LogRecord]) {
    let p = history_path();
    fs::create_dir_all(p.parent().unwrap()).ok();
    serde_json::to_string_pretty(records)
        .ok()
        .and_then(|s| fs::write(&p, s).ok());
}

pub fn add(file: &str, pages: u32, status: &str, duration: &str) {
    let mut records = load();
    records.insert(
        0,
        LogRecord {
            time: chrono_now(),
            file: file.to_string(),
            pages,
            status: status.to_string(),
            duration: duration.to_string(),
        },
    );
    records.truncate(100);
    save(&records);
}

pub fn get_records(limit: usize) -> Vec<LogRecord> {
    let records = load();
    records.into_iter().take(limit).collect()
}

pub fn clear() {
    save(&[]);
}

pub fn cleanup_temp() {
    let dir = temp_dir();
    if dir.is_dir() {
        fs::remove_dir_all(&dir).ok();
    }
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = t.as_secs();
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    let (year, month, day) = days_since_epoch(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02}")
}

fn days_since_epoch(days: i64) -> (i64, i64, i64) {
    let mut y = 1970i64;
    let mut d = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let days_per_month = [
        31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut m = 0i64;
    for (i, &dp) in days_per_month.iter().enumerate() {
        let mut dm = dp;
        if i == 1 && is_leap(y) {
            dm = 29;
        }
        if d < dm {
            m = i as i64 + 1;
            break;
        }
        d -= dm;
    }
    if m == 0 {
        m = 12;
    }
    (y, m, d + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
