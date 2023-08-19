//! Terminal handling.

use crate::error::Result;
use libc::{c_int, c_void, sigaction, sighandler_t, siginfo_t, termios, winsize};
use libc::{SA_SIGINFO, SIGWINCH, STDIN_FILENO, STDOUT_FILENO, TCSADRAIN, TIOCGWINSZ, VMIN, VTIME};
use std::io::{self, Bytes, Read, Stdin};
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// A terminal in raw mode.
pub struct Terminal {
    tty: Bytes<Stdin>,
    prior_term: termios,
}

impl Terminal {
    /// Puts the terminal into raw mode.
    ///
    /// The terminal mode is changed such that raw bytes are read from standard input without
    /// buffering, returning an instance of [`Terminal`] that, when dropped, will restore the
    /// terminal to its prior mode.
    ///
    /// Raw mode is configured such that reads do not block indefinitely when no bytes are
    /// available. In this case, the underlying driver waits `1/10` second before returning with
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if an I/O error occurred while enabling raw mode.
    pub fn new() -> Result<Terminal> {
        register_winsize_handler();

        let prior_term = unsafe {
            let mut prior_term = MaybeUninit::<termios>::uninit();
            os_result(libc::tcgetattr(STDIN_FILENO, prior_term.as_mut_ptr()))?;
            prior_term.assume_init()
        };
        let mut raw_term = prior_term.clone();
        unsafe {
            libc::cfmakeraw(&mut raw_term);
            raw_term.c_cc[VMIN] = 0;
            raw_term.c_cc[VTIME] = 1;
            os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &raw_term))?;
        };
        Ok(Terminal {
            tty: io::stdin().bytes(),
            prior_term,
        })
    }

    /// Reads the next byte if available.
    ///
    /// If a byte is available, this function immediately returns with [`Some<u8>`]. Otherwise, it
    /// will block for `1/10` second, waiting for input, before returning [`None`].
    ///
    /// # Errors
    ///
    /// Returns `Err` if an I/O error occurred while fetching the next byte.
    pub fn read(&mut self) -> Result<Option<u8>> {
        Ok(self.tty.next().transpose()?)
    }

    fn restore(&mut self) -> Result<()> {
        unsafe { os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &self.prior_term)) }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.restore()
            .expect("terminal settings should have been restored");
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
