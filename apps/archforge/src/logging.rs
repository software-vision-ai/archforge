//! The logger, behind the `log` facade.
//!
//! Writes to standard error and to `logs/archforge.log` in the settings directory, rotating at
//! `MAX_BYTES` and keeping `GENERATIONS` older files. Records logged before the directory is known
//! are buffered (up to `BUFFER` records) and written once it is. Directives come from `RUST_LOG`
//! (a level, or `target=level` pairs separated by commas). No path in here panics: every write
//! error is swallowed, so a closed standard-error stream cannot abort the process.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
pub const GENERATIONS: u32 = 2;
pub const BUFFER: usize = 512;

struct Sink {
    dir: Option<PathBuf>,
    file: Option<File>,
    written: u64,
    pending: VecDeque<String>,
}

struct Logger {
    default: LevelFilter,
    directives: Vec<(String, LevelFilter)>,
    sink: Mutex<Sink>,
}

static LOGGER: std::sync::OnceLock<Logger> = std::sync::OnceLock::new();

fn parse_directives(spec: &str) -> (LevelFilter, Vec<(String, LevelFilter)>) {
    let mut default = LevelFilter::Info;
    let mut directives = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('=') {
            Some((target, level)) => {
                if let Ok(l) = level.trim().parse() {
                    directives.push((target.trim().to_string(), l));
                }
            }
            None => {
                if let Ok(l) = part.parse() {
                    default = l;
                }
            }
        }
    }
    (default, directives)
}

impl Logger {
    fn level_for(&self, target: &str) -> LevelFilter {
        self.directives.iter().filter(|(t, _)| target.starts_with(t.as_str())).map(|(_, l)| *l).next_back().unwrap_or(self.default)
    }
}

fn log_path(dir: &Path) -> PathBuf {
    dir.join("archforge.log")
}

impl Sink {
    fn rotate(&mut self) {
        let Some(dir) = self.dir.clone() else { return };
        self.file = None;
        for g in (1..GENERATIONS).rev() {
            let _ = std::fs::rename(dir.join(format!("archforge.log.{g}")), dir.join(format!("archforge.log.{}", g + 1)));
        }
        let _ = std::fs::rename(log_path(&dir), dir.join("archforge.log.1"));
        self.file = OpenOptions::new().create(true).append(true).open(log_path(&dir)).ok();
        self.written = 0;
    }

    fn write(&mut self, line: &str) {
        if self.dir.is_none() {
            if self.pending.len() >= BUFFER {
                self.pending.pop_front();
            }
            self.pending.push_back(line.to_string());
            return;
        }
        if self.written + line.len() as u64 > MAX_BYTES {
            self.rotate();
        }
        if let Some(f) = self.file.as_mut()
            && f.write_all(line.as_bytes()).is_ok()
        {
            self.written += line.len() as u64;
        }
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Ok(mut sink) = self.sink.lock() else { return };
        let text = format!("{} {:<5} {}: {}\n", seconds(), level_name(record.level()), record.target(), record.args());
        let _ = std::io::stderr().write_all(text.as_bytes());
        sink.write(&text);
    }

    fn flush(&self) {
        if let Ok(mut sink) = self.sink.lock()
            && let Some(f) = sink.file.as_mut()
        {
            let _ = f.flush();
        }
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

fn seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Installs the logger. Call it first in `main`, after a `--version` query has been answered.
pub fn init() {
    let spec = std::env::var("RUST_LOG").unwrap_or_default();
    let (default, directives) = parse_directives(&spec);
    let max = directives.iter().map(|(_, l)| *l).chain([default]).max().unwrap_or(LevelFilter::Info);
    let logger =
        LOGGER.get_or_init(|| Logger { default, directives, sink: Mutex::new(Sink { dir: None, file: None, written: 0, pending: VecDeque::new() }) });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(max);
    }
}

/// Points the file sink at `dir` and flushes the records buffered so far.
pub fn set_dir(dir: &Path) {
    let Some(logger) = LOGGER.get() else { return };
    let Ok(mut sink) = logger.sink.lock() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    sink.dir = Some(dir.to_path_buf());
    sink.file = OpenOptions::new().create(true).append(true).open(log_path(dir)).ok();
    sink.written = sink.file.as_ref().and_then(|f| f.metadata().ok()).map(|m| m.len()).unwrap_or(0);
    let pending: Vec<String> = sink.pending.drain(..).collect();
    for line in pending {
        sink.write(&line);
    }
}
