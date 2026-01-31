//! Terminal initialization and interrogation.
//!
//! This module provides functions to initialize the terminal in raw mode such that
//! keystrokes can be read without blocking. It also provides a means of detecting
//! terminal size changes via signal handlers.

use crate::error::{Error, Result};
use libc::{c_int, c_void, sigaction, sighandler_t, siginfo_t, termios, winsize};
use libc::{SA_SIGINFO, SIGWINCH, STDIN_FILENO, STDOUT_FILENO, TCSADRAIN, TIOCGWINSZ, VMIN, VTIME};
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
pub fn init() -> Result<()> {
    default_term().and_then(|mut term| unsafe {
        libc::cfmakeraw(&mut term);
        term.c_cc[VMIN] = 0;
        term.c_cc[VTIME] = 1;
        check_err(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &term))
    })
}

/// Restores the terminal to its original configuration.
pub fn restore() -> Result<()> {
    default_term()
        .and_then(|term| unsafe { check_err(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &term)) })
}

/// Returns the size of the terminal as (rows, cols).
///
/// Calls to this function always query the underlying driver, as the terminal size may
/// have changed since the prior request.
pub fn size() -> Result<(u32, u32)> {
    let win = unsafe {
        let mut win = MaybeUninit::<winsize>::uninit();
        check_err(libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, win.as_mut_ptr()))?;
        win.assume_init()
    };
    Ok((win.ws_row as u32, win.ws_col as u32))
}

/// Returns `true` if the terminal size changed.
///
/// If this function returns `true`, all subsequent calls will return `false` until the
/// terminal size once again changes.
pub fn size_changed() -> bool {
    WINSIZE_CHANGED.swap(false, Ordering::Relaxed)
}

fn check_err(err: c_int) -> Result<()> {
    if err < 0 {
        Err(Error::os())
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
            check_err(libc::tcgetattr(STDIN_FILENO, term.as_mut_ptr()))?;
            term.assume_init()
        };
        Ok(term)
    });
    match def_term {
        Ok(term) => Ok(*term),
        Err(Error::Os { cause }) => Err(Error::os_cloning(cause)),
        Err(e) => panic!("unexpected error: {e}"),
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
        check_err(libc::sigemptyset(&mut (*sigact_ptr).sa_mask))
            .expect("trying to register signal handler");
        (*sigact_ptr).sa_flags = SA_SIGINFO;
        (*sigact_ptr).sa_sigaction = winsize_handler as *const () as sighandler_t;
        check_err(libc::sigaction(SIGWINCH, sigact_ptr, ptr::null_mut()))
            .expect("trying to register signal handler");
    });
}
