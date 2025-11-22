use fork::{daemon, Fork};
use nix::libc::c_int;
use nix::sys::signal::{self, sigaction, SigAction, SigHandler, SigSet, Signal};
use nix::sys::wait::waitpid;
use nix::unistd::Pid;
use std::env::args;
use std::io::{self, BufRead, BufReader, Write};

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static mut CHILD_PID: Pid = Pid::from_raw(-1);
const UNCHECKED_SIGNALS: [Signal; 5] = [
	Signal::SIGTTIN,
	Signal::SIGKILL,
	Signal::SIGSTOP,
	Signal::SIGCHLD,
	Signal::SIGSEGV,
];

extern "C" fn handle_signal(signum: c_int) {
	unsafe {
		assert!(CHILD_PID.as_raw() != -1, "child pid not set!");
		let signal = std::mem::transmute::<i32, Signal>(signum as i32);
		let _ = signal::kill(CHILD_PID, signal);
	}
}

fn get_unix_timestamp() -> u64 {
	let now = SystemTime::now();
	let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
	since_epoch.as_secs()
}

fn process_stream<R: BufRead, W: Write>(reader: R, mut writer: W) {
	for line in reader.lines() {
		if let Ok(line) = line {
			let timestamp = get_unix_timestamp();
			let _ = writeln!(writer, "[{}] {}", timestamp, line);
		}
	}
}

fn main() {
	let args: Vec<String> = args().skip(1).collect();

	if args.is_empty() {
		eprintln!("Usage: qrun <command> [args...]");
		std::process::exit(1);
	}

	match daemon(true, true) {
		Ok(Fork::Child) => {
			// Create a command with piped stdin/stdout/stderr
			let mut cmd = Command::new(&args[0]);
			if args.len() > 1 {
				cmd.args(&args[1..]);
			}

			cmd.stdin(Stdio::piped())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped());

			match cmd.spawn() {
				Ok(mut child) => {
					// Handle stdin
					let stdin = child.stdin.take();
					if let Some(stdin) = stdin {
						let stdin_mutex = Arc::new(Mutex::new(stdin));
						let stdin_clone = Arc::clone(&stdin_mutex);

						thread::spawn(move || {
							let stdin = io::stdin();
							let reader = stdin.lock();

							for line in reader.lines() {
								if let Ok(line) = line {
									if let Ok(mut child_stdin) = stdin_clone.lock() {
										let timestamp = get_unix_timestamp();
										let _ = writeln!(child_stdin, "[{}] {}", timestamp, line);
									}
								}
							}
						});
					}

					// Handle stdout
					if let Some(stdout) = child.stdout.take() {
						let stdout_reader = BufReader::new(stdout);
						thread::spawn(move || {
							process_stream(stdout_reader, io::stdout());
						});
					}

					// Handle stderr
					if let Some(stderr) = child.stderr.take() {
						let stderr_reader = BufReader::new(stderr);
						thread::spawn(move || {
							process_stream(stderr_reader, io::stderr());
						});
					}

					// Wait for the child process to complete
					match child.wait() {
						Ok(status) => std::process::exit(status.code().unwrap_or(1)),
						Err(e) => {
							eprintln!("Failed to wait for child process: {}", e);
							std::process::exit(1);
						}
					}
				}
				Err(e) => {
					eprintln!("Failed to execute command: {}", e);
					std::process::exit(1);
				}
			}
		}
		Ok(Fork::Parent(pid)) => {
			unsafe {
				CHILD_PID = Pid::from_raw(pid);
			}

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

			let _ = waitpid(Pid::from_raw(pid), None);
		}
		Err(e) => {
			eprintln!("qrun failed to fork: {}", e);
			std::process::exit(1);
		}
	}
}
