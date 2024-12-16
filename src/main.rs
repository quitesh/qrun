use fork::{daemon, Fork};
use nix::libc::c_int;
use nix::sys::signal::{self, sigaction, SigAction, SigHandler, SigSet, Signal};
use nix::unistd::Pid;
use nix::sys::wait::waitpid;
use std::env::args;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command;

static mut CHILD_PID: Pid = Pid::from_raw(-1);
const UNCHECKED_SIGNALS: [Signal; 5] = [Signal::SIGTTIN, Signal::SIGKILL, Signal::SIGSTOP, Signal::SIGCHLD, Signal::SIGSEGV];

extern "C" fn handle_signal(signum: c_int) {
	unsafe {
		assert!(CHILD_PID.as_raw() != -1, "child pid not set!");
		let signal = std::mem::transmute::<i32, Signal>(signum as i32);
		let _ = signal::kill(CHILD_PID, signal);
	}
}

fn main() {
	let args: Vec<String> = args().collect();

	match daemon(true, true) {
		Ok(Fork::Child) => {
			//TODO: SIGTTIN handler for input highlighting
			let _ = Command::new("zsh")
				.args(std::iter::once("-c".to_string()).chain(args.into_iter().skip(1)))
				.exec();

			std::process::exit(1);
		}
		Ok(Fork::Parent(pid)) => {
			unsafe {
				CHILD_PID = Pid::from_raw(pid);
			}

			let handler = SigHandler::Handler(handle_signal);

			for sig in 1..31 {
				let signal: Signal = unsafe { std::mem::transmute::<i32, Signal>(sig) };
				if UNCHECKED_SIGNALS.contains(&signal) {
					continue;
				}

				unsafe {
					let _ = sigaction(
						signal,
						&SigAction::new(handler, signal::SaFlags::empty(), SigSet::empty()),
					);
				}
			}

			let _ = waitpid(Pid::from_raw(pid), None);
		}
		Err(e) => {
			std::io::stderr()
				.write_all(format!("qrun failed to fork: {}", e).as_bytes())
				.expect("qrun failed to write to stderr");
			std::process::exit(1);
		}
	}
}
