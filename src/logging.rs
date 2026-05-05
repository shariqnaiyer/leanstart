//! Tee stdout+stderr to both the terminal and a log file.
//!
//! Implemented by replacing fds 1 and 2 with the write end of a pipe and
//! spawning a thread that copies bytes from the read end to (a) the original
//! terminal fd and (b) the log file. This captures Rust `println!`/`eprintln!`
//! AND any subprocess output that inherits the parent's stdout/stderr.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
use std::thread;

use anyhow::{Context, Result, bail};

/// Initialise the tee. Subsequent stdout/stderr writes go to both the terminal
/// and `<run_dir>/run.log`. Returns the log file path so callers can mention
/// it to the user.
pub fn init(run_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(run_dir)
        .with_context(|| format!("Failed to create {}", run_dir.display()))?;

    let log_path = run_dir.join("run.log");
    let log_file = File::create(&log_path)
        .with_context(|| format!("Failed to create {}", log_path.display()))?;

    // Save the original stdout fd so the tee thread can write back to the terminal.
    let orig_stdout = unsafe { libc::dup(libc::STDOUT_FILENO) };
    if orig_stdout < 0 {
        bail!("dup(stdout) failed");
    }

    // Create the pipe whose write end we'll splice onto stdout/stderr.
    let mut pipe_fds = [0i32; 2];
    if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } != 0 {
        bail!("pipe() failed");
    }
    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    // Redirect both stdout (1) and stderr (2) into the pipe. We collapse the
    // two streams since the tee thread can't distinguish them once mixed —
    // acceptable for an orchestration log.
    if unsafe { libc::dup2(write_fd, libc::STDOUT_FILENO) } < 0 {
        bail!("dup2(stdout) failed");
    }
    if unsafe { libc::dup2(write_fd, libc::STDERR_FILENO) } < 0 {
        bail!("dup2(stderr) failed");
    }
    unsafe { libc::close(write_fd) };

    thread::spawn(move || {
        let mut reader = unsafe { File::from_raw_fd(read_fd) };
        let mut term = unsafe { File::from_raw_fd(orig_stdout) };
        let mut log = log_file;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = term.write_all(&buf[..n]);
                    let _ = term.flush();
                    let _ = log.write_all(&buf[..n]);
                    let _ = log.flush();
                }
            }
        }
    });

    Ok(log_path)
}

/// Flush stdout/stderr and give the tee thread a moment to drain the pipe
/// before the process exits. Call this at the end of long-running commands.
pub fn shutdown() {
    use std::io::Write as _;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    // Tiny delay so the tee thread can flush the last writes to disk.
    thread::sleep(std::time::Duration::from_millis(50));
}
