use qjournal::JournalWriter;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use libc;

#[cfg(unix)]
use nix::libc::c_int;
#[cfg(unix)]
use nix::sys::signal::{self, sigaction, SigAction, SigHandler, SigSet, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

#[cfg(unix)]
static mut CHILD_PID: Pid = Pid::from_raw(-1);

#[cfg(unix)]
const UNCHECKED_SIGNALS: [Signal; 5] = [
    Signal::SIGKILL,
    Signal::SIGSTOP,
    Signal::SIGCHLD,
    Signal::SIGSEGV,
    Signal::SIGTTIN, // We handle this one specially
];

pub struct RunConfig {
    pub journal: PathBuf,
    pub shell: Option<String>,
    pub command: Vec<String>,
}

#[cfg(unix)]
extern "C" fn handle_signal(signum: c_int) {
    unsafe {
        let signal = std::mem::transmute::<i32, Signal>(signum as i32);

        match signal {
            Signal::SIGTTIN => {
                // Input requested - alert the user
                alert_input_requested();
            }
            _ => {
                // Forward other signals to child process
                if CHILD_PID.as_raw() != -1 {
                    let _ = signal::kill(CHILD_PID, signal);
                }
            }
        }
    }
}

#[allow(dead_code)]
#[cfg(target_os = "windows")]
fn alert_input_requested() {
    use std::ffi::CString;
    use winapi::um::winuser::{MessageBoxA, MB_ICONEXCLAMATION, MB_OK};

    let title = CString::new("qrun - Input Requested").unwrap();
    let message = CString::new("The running process is requesting input!").unwrap();

    unsafe {
        MessageBoxA(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONEXCLAMATION,
        );
    }
}

#[allow(dead_code)]
#[cfg(all(unix, not(target_os = "windows")))]
fn alert_input_requested() {
    // Try various cross-platform notification methods
    if let Err(_) = Command::new("notify-send")
        .args(&["qrun - Input Requested", "The running process is requesting input!"])
        .output()
    {
        // Fallback to terminal bell and message
        print!("\x07"); // ASCII bell character
        eprintln!("🔔 INPUT REQUESTED: The running process is waiting for input!");
        let _ = std::io::stderr().flush();
    }
}

pub fn get_default_shell() -> String {
    #[cfg(target_os = "windows")]
    return "cmd".to_string();

    #[cfg(unix)]
    return "zsh".to_string();
}

pub fn get_shell_args(_shell: &str, command: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    if _shell == "cmd" {
        return vec!["/C".to_string(), command.to_string()];
    }

    // Default Unix-style shell args
    vec!["-c".to_string(), command.to_string()]
}

pub fn write_journal_entry(
    writer: &mut JournalWriter,
    message: &str,
    priority: &str,
    stream: &str,
    pid: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    writer.append_entry(&[
        ("MESSAGE", message.as_bytes()),
        ("PRIORITY", priority.as_bytes()),
        ("SYSLOG_IDENTIFIER", b"qrun"),
        ("_PID", pid.to_string().as_bytes()),
        ("_STREAM", stream.as_bytes()),
        ("_TIMESTAMP", timestamp.to_string().as_bytes()),
    ])?;

    Ok(())
}

pub fn monitor_process_output(
    mut child: std::process::Child,
    journal_writer: JournalWriter,
    process_name: String,
) -> Result<i32, Box<dyn std::error::Error>> {
    let pid = child.id();

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    let journal_writer = Arc::new(std::sync::Mutex::new(journal_writer));
    let stdout_writer = Arc::clone(&journal_writer);
    let stderr_writer = Arc::clone(&journal_writer);

    let stdout_handle = thread::spawn(move || {
        for line in stdout_reader.lines() {
            match line {
                Ok(line) => {
                    println!("{}", line);
                    if let Ok(mut writer) = stdout_writer.lock() {
                        let _ = write_journal_entry(&mut writer, &line, "6", "stdout", pid);
                    }
                }
                Err(_) => break,
            }
        }
    });

    let stderr_handle = thread::spawn(move || {
        for line in stderr_reader.lines() {
            match line {
                Ok(line) => {
                    eprintln!("{}", line);
                    if let Ok(mut writer) = stderr_writer.lock() {
                        let _ = write_journal_entry(&mut writer, &line, "3", "stderr", pid);
                    }
                }
                Err(_) => break,
            }
        }
    });

    #[cfg(unix)]
    unsafe {
        CHILD_PID = Pid::from_raw(pid as i32);
    }

    let exit_status = child.wait()?;
    let exit_code = exit_status.code().unwrap_or(-1);

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if let Ok(mut writer) = journal_writer.lock() {
        let final_message = if exit_code == 0 {
            format!("Process '{}' completed successfully", process_name)
        } else {
            format!("Process '{}' exited with code {}", process_name, exit_code)
        };

        let priority = if exit_code == 0 { "6" } else { "3" };
        let _ = write_journal_entry(&mut writer, &final_message, priority, "qrun", pid);
        let _ = writer.flush();
    }

    Ok(exit_code)
}

#[cfg(unix)]
pub fn setup_signal_handlers() -> Result<(), Box<dyn std::error::Error>> {
    let handler = SigHandler::Handler(handle_signal);
    for sig in Signal::iterator() {
        if UNCHECKED_SIGNALS.contains(&sig) {
            continue;
        }
        unsafe {
            let _ = sigaction(
                sig,
                &SigAction::new(handler, signal::SaFlags::empty(), SigSet::empty()),
            );
        }
    }

    unsafe {
        let _ = sigaction(
            Signal::SIGTTIN,
            &SigAction::new(handler, signal::SaFlags::empty(), SigSet::empty()),
        );
    }

    Ok(())
}

#[cfg(not(unix))]
pub fn setup_signal_handlers() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

// ── Unix PTY hotplug API ──────────────────────────────────────────────────────

/// Handle returned by [`spawn_with_pty_detect`].
#[cfg(unix)]
pub struct PtyDetectHandle {
    /// The spawned child process (tokio handle for async wait).
    pub child: tokio::process::Child,
    /// Parent end of the socketpair used for PTY negotiation.
    pub sock: std::os::unix::net::UnixStream,
}

/// Spawn `shell shell_args` with piped stdout/stderr and an LD_PRELOAD shim
/// that intercepts the first `isatty()` call.  The shim blocks until the
/// parent calls [`complete_pty_handshake`], which sends the slave PTY fd over
/// the socketpair.
#[cfg(unix)]
pub fn spawn_with_pty_detect(
    shell: &str,
    shell_args: &[String],
    cwd: &std::path::Path,
    env: &std::collections::HashMap<String, String>,
) -> Result<PtyDetectHandle, Box<dyn std::error::Error>> {
    use std::os::unix::io::AsRawFd;
    use std::os::unix::net::UnixStream;

    let (parent_sock, child_sock) = UnixStream::pair()?;
    let child_fd = child_sock.as_raw_fd();

    // Clear O_CLOEXEC on the child end so it survives exec
    unsafe {
        libc::fcntl(child_fd, libc::F_SETFD, 0);
    }

    #[cfg(target_os = "macos")]
    let preload_key = "DYLD_INSERT_LIBRARIES";
    #[cfg(not(target_os = "macos"))]
    let preload_key = "LD_PRELOAD";

    let child = tokio::process::Command::new(shell)
        .args(shell_args)
        .current_dir(cwd)
        .envs(env)
        .env(preload_key, env!("QRUN_DETECT_SO"))
        .env("QRUN_SOCK_FD", child_fd.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    drop(child_sock);
    Ok(PtyDetectHandle { child, sock: parent_sock })
}

/// Called after receiving `b'P'` on the socket: allocates a PTY, sends the
/// slave fd to the child via SCM_RIGHTS, and returns the master.
///
/// **The socket must be in blocking mode** when this function is called.
#[cfg(unix)]
pub fn complete_pty_handshake(
    sock: &std::os::unix::net::UnixStream,
    rows: u16,
    cols: u16,
) -> Result<pty_process::Pty, Box<dyn std::error::Error>> {
    use std::os::unix::io::AsRawFd;

    let pty = pty_process::Pty::new()?;
    pty.resize(pty_process::Size::new(rows, cols))?;

    // pty_process::Pts doesn't expose AsRawFd publicly, so we open the slave
    // device ourselves via ptsname.
    let slave_fd = open_slave_fd(pty.as_raw_fd())?;
    send_fd_scm(sock.as_raw_fd(), slave_fd)?;
    unsafe { libc::close(slave_fd) };

    Ok(pty)
}

/// Open the slave (pts) device for a master PTY fd.
/// Returns a newly-opened file descriptor for the slave; caller must close it.
#[cfg(unix)]
fn open_slave_fd(master_fd: libc::c_int) -> Result<libc::c_int, Box<dyn std::error::Error>> {
    let slave_path: std::ffi::CString = unsafe {
        #[cfg(target_os = "linux")]
        {
            let mut buf = vec![0u8; 64];
            let ret = libc::ptsname_r(
                master_fd,
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
            );
            if ret != 0 {
                return Err(
                    format!("ptsname_r: {}", std::io::Error::last_os_error()).into(),
                );
            }
            std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char)
                .to_owned()
        }
        #[cfg(not(target_os = "linux"))]
        {
            let ptr = libc::ptsname(master_fd);
            if ptr.is_null() {
                return Err("ptsname returned null".into());
            }
            std::ffi::CStr::from_ptr(ptr).to_owned()
        }
    };

    let fd = unsafe { libc::open(slave_path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(format!("open pts: {}", std::io::Error::last_os_error()).into());
    }
    Ok(fd)
}

/// Write raw PTY output (including ANSI escape sequences) to the journal.
/// No-op on non-Unix platforms.
pub fn journal_pty_output(
    writer: &mut JournalWriter,
    raw: &[u8],
    pid: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let trimmed = raw.trim_ascii_end();
        if !trimmed.is_empty() {
            write_journal_entry(writer, &String::from_utf8_lossy(trimmed), "6", "stdout", pid)?;
        }
    }
    Ok(())
}

/// Send a file descriptor over a Unix domain socket using SCM_RIGHTS.
#[cfg(unix)]
fn send_fd_scm(sock: libc::c_int, fd: libc::c_int) -> Result<(), Box<dyn std::error::Error>> {
    use std::mem;

    let mut iov_buf: u8 = 0;
    let mut iov = libc::iovec {
        iov_base: &mut iov_buf as *mut u8 as *mut libc::c_void,
        iov_len: 1,
    };

    let cmsg_space = unsafe { libc::CMSG_SPACE(mem::size_of::<libc::c_int>() as u32) } as usize;
    let mut cmsg_buf = vec![0u8; cmsg_space];

    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov as *mut libc::iovec;
    msg.msg_iovlen = 1 as _;
    msg.msg_control = cmsg_buf.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cmsg_space as _;

    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&mut msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as _;
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = fd;

        let ret = libc::sendmsg(sock, &msg, 0);
        if ret < 0 {
            return Err(
                format!("sendmsg failed: {}", std::io::Error::last_os_error()).into(),
            );
        }
    }

    Ok(())
}

pub fn run(config: RunConfig) -> Result<i32, Box<dyn std::error::Error>> {
    let RunConfig { journal, shell, command } = config;

    let mut journal_writer = JournalWriter::open(&journal)
        .map_err(|e| format!("Failed to open journal file '{}': {}", journal.display(), e))?;

    let shell = shell.unwrap_or_else(get_default_shell);
    let full_command = command.join(" ");
    let process_name = full_command.clone();

    write_journal_entry(
        &mut journal_writer,
        &format!("Starting command: {} {}", shell, full_command),
        "6",
        "qrun",
        std::process::id(),
    )?;
    journal_writer.flush()?;

    setup_signal_handlers()?;

    let shell_args = get_shell_args(&shell, &full_command);
    let child = Command::new(&shell)
        .args(&shell_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start process: {}", e))?;

    monitor_process_output(child, journal_writer, process_name)
}
