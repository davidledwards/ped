use crate::error::Result;
use libc::{c_int, c_void, sigaction, sighandler_t, siginfo_t, termios, winsize};
use libc::{SA_SIGINFO, SIGWINCH, STDIN_FILENO, STDOUT_FILENO, TCSADRAIN, TIOCGWINSZ, VMIN, VTIME};
use std::io;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

pub type Terminal = termios;

pub fn init() -> Result<Terminal> {
    // Fetch current terminal info for restoration.
    let term = unsafe {
        let mut term = MaybeUninit::<termios>::uninit();
        os_result(libc::tcgetattr(STDIN_FILENO, term.as_mut_ptr()))?;
        term.assume_init()
    };

    // Shift terminal into raw mode so keystrokes can be read as typed.
    // Note that reads will timeout after 1/10 second if waiting for data.
    let mut raw_term = term.clone();
    unsafe {
        libc::cfmakeraw(&mut raw_term);
        raw_term.c_cc[VMIN] = 0;
        raw_term.c_cc[VTIME] = 1;
        os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, &raw_term))?;
    };

    // Register callback to report when size changes.
    unsafe {
        let mut sigact = MaybeUninit::<sigaction>::uninit();
        let sigact_ptr = sigact.as_mut_ptr();
        os_result(libc::sigemptyset(&mut (*sigact_ptr).sa_mask))?;
        (*sigact_ptr).sa_flags = SA_SIGINFO;
        (*sigact_ptr).sa_sigaction = winsize_handler as sighandler_t;
        os_result(libc::sigaction(SIGWINCH, sigact_ptr, ptr::null_mut()))?;
    }

    Ok(term)
}

pub fn size() -> Result<(u32, u32)> {
    let win = unsafe {
        let mut win = MaybeUninit::<winsize>::uninit();
        os_result(libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, win.as_mut_ptr()))
            .map(|_| win.assume_init())?
    };
    Ok((win.ws_row as u32, win.ws_col as u32))
}

pub fn size_changed() -> bool {
    WINSIZE_CHANGED.swap(false, Ordering::Relaxed)
}

pub fn restore(term: &Terminal) -> Result<()> {
    unsafe {
        os_result(libc::tcsetattr(STDIN_FILENO, TCSADRAIN, term))?;
    }
    Ok(())
}

fn os_result(err: c_int) -> Result<()> {
    if err < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

static WINSIZE_CHANGED: AtomicBool = AtomicBool::new(false);

extern "C" fn winsize_handler(_: c_int, _: *mut siginfo_t, _: *mut c_void) {
    WINSIZE_CHANGED.store(true, Ordering::Relaxed);
}
