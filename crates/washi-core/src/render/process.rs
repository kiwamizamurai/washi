use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
pub const TIMEOUT_ENV: &str = "WASHI_COMPILE_TIMEOUT";

const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub fn timeout_from_env() -> Duration {
    timeout_from(std::env::var(TIMEOUT_ENV).ok().as_deref())
}

fn timeout_from(value: Option<&str>) -> Duration {
    value
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&secs| secs > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_TIMEOUT)
}

#[derive(Debug)]
pub enum RunError {
    Spawn(std::io::Error),
    TimedOut(Duration),
    Cancelled,
}

pub struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn run(command: &mut Command, timeout: Duration, cancel: &AtomicBool) -> Result<Captured, RunError> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);

    let mut child = command.spawn().map_err(RunError::Spawn)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                stop(&mut child);
                return Err(RunError::Spawn(e));
            }
        }
        if cancel.load(Ordering::Relaxed) {
            stop(&mut child);
            return Err(RunError::Cancelled);
        }
        if started.elapsed() >= timeout {
            stop(&mut child);
            return Err(RunError::TimedOut(timeout));
        }
        thread::sleep(POLL_INTERVAL);
    };

    Ok(Captured {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

fn stop(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

type Flags = HashMap<PathBuf, Arc<AtomicBool>>;

static RUNNING: LazyLock<Mutex<Flags>> = LazyLock::new(Default::default);

pub struct Job {
    key: PathBuf,
    flag: Arc<AtomicBool>,
}

impl Job {
    pub fn start(source: &Path) -> Self {
        let flag = Arc::new(AtomicBool::new(false));
        let previous = RUNNING.lock().unwrap().insert(source.to_path_buf(), flag.clone());
        if let Some(previous) = previous {
            previous.store(true, Ordering::Relaxed);
        }
        Self { key: source.to_path_buf(), flag }
    }

    pub fn cancelled(&self) -> &AtomicBool {
        &self.flag
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        let mut running = RUNNING.lock().unwrap();
        if running.get(&self.key).is_some_and(|f| Arc::ptr_eq(f, &self.flag)) {
            running.remove(&self.key);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    fn never() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn alive(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[test]
    fn captures_output_and_status() {
        let out = run(&mut sh("echo out; echo err >&2; exit 3"), Duration::from_secs(5), &never()).unwrap();
        assert_eq!(out.status.code(), Some(3));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "out");
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "err");
    }

    #[test]
    fn does_not_deadlock_on_large_output() {
        let out = run(&mut sh("head -c 3000000 /dev/zero"), Duration::from_secs(10), &never()).unwrap();
        assert_eq!(out.stdout.len(), 3_000_000);
    }

    #[test]
    fn stdin_is_closed_so_a_prompt_cannot_hang_the_build() {
        let out = run(&mut sh("cat; echo done"), Duration::from_secs(5), &never()).unwrap();
        assert!(out.status.success());
    }

    #[test]
    fn times_out_and_returns_promptly() {
        let started = Instant::now();
        let err = run(&mut sh("sleep 30"), Duration::from_millis(200), &never()).err().unwrap();
        assert!(matches!(err, RunError::TimedOut(_)));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn timeout_also_stops_grandchildren() {
        let pid_file = std::env::temp_dir().join(format!("washi-pid-{}", std::process::id()));
        let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
        let err = run(&mut sh(&script), Duration::from_millis(500), &never()).err().unwrap();
        assert!(matches!(err, RunError::TimedOut(_)));

        let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
        let _ = std::fs::remove_file(&pid_file);
        for _ in 0..100 {
            if !alive(pid) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("孫プロセス {pid} が残っています");
    }

    #[test]
    fn cancel_flag_stops_the_command() {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            flag.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        let err = run(&mut sh("sleep 30"), Duration::from_secs(60), &cancel).err().unwrap();
        canceller.join().unwrap();
        assert!(matches!(err, RunError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn spawn_failure_is_reported() {
        let err = run(&mut Command::new("/nonexistent/washi-tool"), Duration::from_secs(1), &never()).err().unwrap();
        assert!(matches!(err, RunError::Spawn(_)));
    }

    #[test]
    fn a_new_job_for_the_same_file_cancels_the_old_one() {
        let path = Path::new("/jobs/same.tex");
        let old = Job::start(path);
        assert!(!old.cancelled().load(Ordering::Relaxed));
        let new = Job::start(path);
        assert!(old.cancelled().load(Ordering::Relaxed));
        assert!(!new.cancelled().load(Ordering::Relaxed));
        drop(old);
        let newest = Job::start(path);
        assert!(new.cancelled().load(Ordering::Relaxed));
        drop(newest);
    }

    #[test]
    fn jobs_for_different_files_do_not_interfere() {
        let a = Job::start(Path::new("/jobs/a.tex"));
        let _b = Job::start(Path::new("/jobs/b.tex"));
        assert!(!a.cancelled().load(Ordering::Relaxed));
    }

    #[test]
    fn timeout_setting_falls_back_to_the_default() {
        assert_eq!(timeout_from(None), DEFAULT_TIMEOUT);
        assert_eq!(timeout_from(Some("30")), Duration::from_secs(30));
        assert_eq!(timeout_from(Some(" 45 ")), Duration::from_secs(45));
        assert_eq!(timeout_from(Some("0")), DEFAULT_TIMEOUT);
        assert_eq!(timeout_from(Some("abc")), DEFAULT_TIMEOUT);
    }
}
