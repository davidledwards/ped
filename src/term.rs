//! Terminal handling.

use crate::error::{Error, Result};
use libc::{c_int, c_void, sigaction, sighandler_t, siginfo_t, termios, winsize};
use libc::{SA_SIGINFO, SIGWINCH, STDIN_FILENO, STDOUT_FILENO, TCSADRAIN, TIOCGWINSZ, VMIN, VTIME};
use std::io;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Puts the terminal into raw mode.
///
/// The terminal mode is changed such that raw bytes are read from standard input without
/// buffering. Raw mode is configured such that reads do not block indefinitely when no
/// bytes are available. In this case, the underlying driver waits `1/10` second before
/// returning with nothing.
///
/// # Errors
///
/// Returns [`Err`] if an I/O error occurs while configuring raw mode.
pub fn init() -> Result<()> {
    match default_term() {
        Ok(mut term) => {
            unsafe {
                libc::cfmakeraw(&mut term);
                term.c_cc[VMIN] = 0;
                term.c_cc[VTIME] = 1;
                os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &term))?;
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Restores the terminal to its original configuration.
///
/// # Errors
///
/// Returns [`Err`] if an I/O error occurs while restoring the terminal configuration.
pub fn restore() -> Result<()> {
    match default_term() {
        Ok(term) => {
            unsafe { os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &term))? }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Returns the size of the terminal as (rows, cols).
///
/// Calls to this function always query the underlying driver, as the terminal size may have
/// changed since the prior request.
///
/// # Errors
///
/// Returns [`Err`] if an I/O error occurred.
pub fn size() -> Result<(u32, u32)> {
    let win = unsafe {
        let mut win = MaybeUninit::<winsize>::uninit();
        os_result(libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, win.as_mut_ptr()))?;
        win.assume_init()
    };
    Ok((win.ws_row as u32, win.ws_col as u32))
}

/// Returns `true` if the terminal size changed.
///
/// If this function returns `true`, all subsequent calls will return `false` until the terminal
/// size once again changes.
pub fn size_changed() -> bool {
    WINSIZE_CHANGED.swap(false, Ordering::Relaxed)
}

fn os_result(err: c_int) -> Result<()> {
    if err < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

/// Ensures that default terminal configuration is captured at most once.
static DEFAULT_TERM: OnceLock<Result<termios>> = OnceLock::new();

/// Returns the default terminal configuration that can be used for restoring after
/// changing into raw mode.
fn default_term() -> Result<termios> {
    let def_term = DEFAULT_TERM.get_or_init(|| {
        register_winsize_handler();
        let term = unsafe {
            let mut term = MaybeUninit::<termios>::uninit();
            os_result(libc::tcgetattr(STDIN_FILENO, term.as_mut_ptr()))?;
            term.assume_init()
        };
        Ok(term)
    });
    match def_term {
        Err(Error::IO(e)) => Err(io::Error::new(e.kind(), e.to_string()).into()),
        Err(e) => panic!("{:?}", e),
        Ok(term) => Ok(term.clone()),
    }
}

/// Ensures that signal handler is registered at most once.
static WINSIZE_HANDLER: OnceLock<()> = OnceLock::new();

/// Used by signal handler to convey that the terminal size changed.
static WINSIZE_CHANGED: AtomicBool = AtomicBool::new(false);

/// Signal handler that gets invoked when the terminal size changes.
extern "C" fn winsize_handler(_: c_int, _: *mut siginfo_t, _: *mut c_void) {
    WINSIZE_CHANGED.store(true, Ordering::Relaxed);
}

/// Registers the signal handler to capture changes in terminal size.
fn register_winsize_handler() {
    WINSIZE_HANDLER.get_or_init(|| unsafe {
        let mut sigact = MaybeUninit::<sigaction>::uninit();
        let sigact_ptr = sigact.as_mut_ptr();
        os_result(libc::sigemptyset(&mut (*sigact_ptr).sa_mask)).expect("register signal handler");
        (*sigact_ptr).sa_flags = SA_SIGINFO;
        (*sigact_ptr).sa_sigaction = winsize_handler as sighandler_t;
        os_result(libc::sigaction(SIGWINCH, sigact_ptr, ptr::null_mut()))
            .expect("register signal handler");
    });
}
