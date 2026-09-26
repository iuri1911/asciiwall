use anyhow::{Result, bail};
use std::path::PathBuf;

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn pid_file() -> PathBuf {
    runtime_dir().join("asciiwall.pid")
}

fn request_file() -> PathBuf {
    runtime_dir().join("asciiwall.request")
}

/// Pid of a live daemon; removes a stale pidfile.
pub fn running_pid() -> Option<i32> {
    let path = pid_file();
    let pid: i32 = std::fs::read_to_string(&path).ok()?.trim().parse().ok()?;
    let alive = unsafe { libc::kill(pid, 0) } == 0
        && std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "asciiwall");
    if alive {
        Some(pid)
    } else {
        let _ = std::fs::remove_file(path);
        None
    }
}

pub fn claim_pidfile() -> Result<()> {
    let me = std::process::id() as i32;
    if let Some(pid) = running_pid().filter(|p| *p != me) {
        bail!("already running (pid {pid})");
    }
    std::fs::write(pid_file(), me.to_string())?;
    Ok(())
}

pub fn release_pidfile() {
    let _ = std::fs::remove_file(pid_file());
}

/// Ask the daemon to rotate (to the requested scene if one was written).
pub fn signal_daemon(pid: i32) -> Result<()> {
    if unsafe { libc::kill(pid, libc::SIGUSR1) } != 0 {
        bail!("cannot signal daemon (pid {pid}): {}", std::io::Error::last_os_error());
    }
    Ok(())
}

pub fn write_request(id: &str) -> Result<()> {
    std::fs::write(request_file(), id)?;
    Ok(())
}

pub fn take_request() -> Option<String> {
    let path = request_file();
    let id = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(path);
    Some(id.trim().to_string()).filter(|s| !s.is_empty())
}
