// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Small rotating text log (REQ-PROC-001). It records events and lengths, never clipboard
//! text (REQ-SEC-001): callers pass only fixed messages and numbers.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_BYTES: u64 = 256 * 1024;

static LOG: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(path: PathBuf) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    *LOG.lock().unwrap_or_else(|e| e.into_inner()) = Some(path);
}

/// "2026-11-21 21:30:05" (UTC) from Unix seconds.
pub fn format_time(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil-from-days (proleptic Gregorian), after H. Hinnant
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn write_line(path: &PathBuf, level: &str, msg: &str) {
    if fs::metadata(path).map(|m| m.len() >= MAX_BYTES).unwrap_or(false) {
        let mut old = path.clone().into_os_string();
        old.push(".1");
        let _ = fs::rename(path, PathBuf::from(old));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let file: std::io::Result<File> = OpenOptions::new().create(true).append(true).open(path);
    if let Ok(mut f) = file {
        let _ = writeln!(f, "{} {level:5} {msg}", format_time(now));
    }
}

pub fn log(level: &str, msg: &str) {
    if let Some(path) = LOG.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        write_line(path, level, msg);
    }
}

pub fn info(msg: &str) {
    log("INFO", msg);
}
pub fn warn(msg: &str) {
    log("WARN", msg);
}
pub fn error(msg: &str) {
    log("ERROR", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format() {
        assert_eq!(format_time(0), "1970-01-01 00:00:00");
        assert_eq!(format_time(1_700_000_000), "2023-11-14 22:13:20");
        assert_eq!(format_time(1_782_000_000), "2026-06-21 00:00:00");
        assert_eq!(format_time(951_782_400), "2000-02-29 00:00:00");
    }

    #[test]
    fn writes_and_rotates() {
        let dir = std::env::temp_dir().join(format!("re-type-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("re-type.log");
        write_line(&path, "INFO", "hello");
        assert!(fs::read_to_string(&path).unwrap().contains("INFO  hello"));
        fs::write(&path, vec![b'x'; MAX_BYTES as usize]).unwrap();
        write_line(&path, "WARN", "after rotation");
        assert!(dir.join("re-type.log.1").exists());
        let new = fs::read_to_string(&path).unwrap();
        assert!(new.contains("after rotation") && new.len() < 100);
        let _ = fs::remove_dir_all(&dir);
    }
}
