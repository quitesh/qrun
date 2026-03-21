use qjournal::JournalWriter;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

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
