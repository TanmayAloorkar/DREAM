//! Runs one shell command at a time and streams its output into a shared log.
//! A single slot is deliberate: two heavy DREAM jobs at once can exhaust RAM on a 15 GB laptop.

use eframe::egui;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_LINES: usize = 4000;

pub struct Running {
    pub name: String,
    pub started: Instant,
    child: Child,
}

pub struct Finished {
    pub name: String,
    pub code: Option<i32>,
    pub elapsed: Duration,
    pub stopped: bool,
}

#[derive(Default)]
pub struct Jobs {
    pub running: Option<Running>,
    pub last: Option<Finished>,
    pub log: Arc<Mutex<VecDeque<String>>>,
    stop_requested: bool,
}

impl Jobs {
    pub fn busy(&self) -> Option<&str> {
        self.running.as_ref().map(|r| r.name.as_str())
    }

    pub fn start(&mut self, ctx: &egui::Context, name: &str, script: &str, cwd: &Path, env: &[(&str, String)]) {
        if self.running.is_some() {
            return;
        }
        self.stop_requested = false;
        self.push(format!("$ {script}"));
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(format!("set -eo pipefail\n{script}"))
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // own process group, so Stop also reaches Nextflow, Java and R children
            .process_group(0);
        for (key, value) in env {
            cmd.env(key, value);
        }
        match cmd.spawn() {
            Ok(mut child) => {
                if let Some(out) = child.stdout.take() {
                    self.pipe(ctx, out);
                }
                if let Some(err) = child.stderr.take() {
                    self.pipe(ctx, err);
                }
                self.running = Some(Running { name: name.into(), started: Instant::now(), child });
            }
            Err(e) => {
                self.push(format!("Could not start bash: {e}"));
                self.last = Some(Finished { name: name.into(), code: None, elapsed: Duration::ZERO, stopped: false });
            }
        }
    }

    /// Returns true once, on the frame the running job ends.
    pub fn poll(&mut self) -> bool {
        let Some(run) = self.running.as_mut() else { return false };
        match run.child.try_wait() {
            Ok(Some(status)) => {
                let run = self.running.take().expect("checked above");
                self.last = Some(Finished {
                    name: run.name,
                    code: status.code(),
                    elapsed: run.started.elapsed(),
                    stopped: self.stop_requested,
                });
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.push(format!("Lost track of the job: {e}"));
                false
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(run) = &self.running {
            self.stop_requested = true;
            let group = format!("-{}", run.child.id());
            let _ = Command::new("kill").args(["-TERM", "--", &group]).status();
            self.push("Stop requested: sent SIGTERM to the job's process group".into());
        }
    }

    pub fn clear(&mut self) {
        self.log.lock().unwrap().clear();
    }

    fn push(&self, line: String) {
        push_line(&self.log, line);
    }

    fn pipe(&self, ctx: &egui::Context, stream: impl Read + Send + 'static) {
        let log = Arc::clone(&self.log);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                push_line(&log, strip_ansi(&line));
                ctx.request_repaint();
            }
        });
    }
}

fn push_line(log: &Mutex<VecDeque<String>>, line: String) {
    let mut log = log.lock().unwrap();
    // carriage-return progress bars: keep only the last segment
    let line = line.rsplit('\r').next().unwrap_or("").to_string();
    log.push_back(line);
    while log.len() > MAX_LINES {
        log.pop_front();
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // skip "ESC [ ... letter"
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn elapsed(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_colour_codes_and_keeps_text() {
        assert_eq!(strip_ansi("\x1b[32mOK\x1b[0m done"), "OK done");
    }

    #[test]
    fn keeps_last_carriage_return_segment() {
        let log = Mutex::new(VecDeque::new());
        push_line(&log, "10%\r50%\r100%".into());
        assert_eq!(log.lock().unwrap().back().unwrap(), "100%");
    }

    fn wait(jobs: &mut Jobs) {
        let start = Instant::now();
        while !jobs.poll() {
            assert!(start.elapsed() < Duration::from_secs(10), "job did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_one_job_and_reports_its_exit_code() {
        let ctx = egui::Context::default();
        let mut jobs = Jobs::default();
        jobs.start(&ctx, "Echo", "echo hello; exit 3", Path::new("/tmp"), &[]);
        // a second start while busy is ignored
        jobs.start(&ctx, "Other", "echo nope", Path::new("/tmp"), &[]);
        assert_eq!(jobs.busy(), Some("Echo"));
        wait(&mut jobs);
        std::thread::sleep(Duration::from_millis(50)); // reader threads drain
        let last = jobs.last.as_ref().unwrap();
        assert_eq!((last.name.as_str(), last.code, last.stopped), ("Echo", Some(3), false));
        let log = jobs.log.lock().unwrap();
        assert!(log.iter().any(|l| l == "hello"));
        assert!(!log.iter().any(|l| l.contains("nope")));
    }

    #[test]
    fn stop_ends_the_whole_process_group() {
        let ctx = egui::Context::default();
        let mut jobs = Jobs::default();
        // the child sleep would outlive a plain kill of bash
        jobs.start(&ctx, "Sleep", "sleep 30 & wait", Path::new("/tmp"), &[]);
        std::thread::sleep(Duration::from_millis(200));
        jobs.stop();
        wait(&mut jobs);
        assert!(jobs.last.as_ref().unwrap().stopped);
    }

    #[test]
    fn formats_elapsed_time() {
        assert_eq!(elapsed(Duration::from_secs(75)), "1:15");
        assert_eq!(elapsed(Duration::from_secs(3725)), "1:02:05");
    }
}
