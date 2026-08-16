use std::{
    cell::{Cell, RefCell},
    fs::{File, OpenOptions},
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use nix::{
    errno::Errno,
    sys::{
        signal::{SigSet, SigmaskHow, Signal, killpg},
        termios::{SetArg, Termios, tcgetattr, tcsetattr},
    },
    unistd::{Pid, getpgrp, tcgetpgrp, tcsetpgrp},
};
use signal_hook::{SigId, consts::signal, flag, low_level};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalState {
    Shell,
    Foreground { pgid: Pid },
}

#[derive(Debug)]
struct SignalFlags {
    interrupt: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    input: Arc<AtomicBool>,
    output: Arc<AtomicBool>,
    resize: Arc<AtomicBool>,
    registrations: Vec<SigId>,
}

impl SignalFlags {
    fn install() -> io::Result<Self> {
        let interrupt = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let input = Arc::new(AtomicBool::new(false));
        let output = Arc::new(AtomicBool::new(false));
        let resize = Arc::new(AtomicBool::new(false));
        let mut registrations = Vec::new();
        for (signal, target) in [
            (signal::SIGINT, Arc::clone(&interrupt)),
            (signal::SIGTSTP, Arc::clone(&stop)),
            (signal::SIGTTIN, Arc::clone(&input)),
            (signal::SIGTTOU, Arc::clone(&output)),
            (signal::SIGWINCH, Arc::clone(&resize)),
        ] {
            match flag::register(signal, target) {
                Ok(registration) => registrations.push(registration),
                Err(error) => {
                    for registration in registrations {
                        low_level::unregister(registration);
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            interrupt,
            stop,
            input,
            output,
            resize,
            registrations,
        })
    }

    fn clear(&self) {
        for flag in [
            &self.interrupt,
            &self.stop,
            &self.input,
            &self.output,
            &self.resize,
        ] {
            flag.store(false, Ordering::Release);
        }
    }

    fn forward_pending(&self, pgid: Pid) {
        for (flag, signal) in [
            (&self.interrupt, Signal::SIGINT),
            (&self.stop, Signal::SIGTSTP),
            (&self.resize, Signal::SIGWINCH),
        ] {
            if flag.swap(false, Ordering::AcqRel) {
                let _ = killpg(pgid, signal);
            }
        }
        self.input.store(false, Ordering::Release);
        self.output.store(false, Ordering::Release);
    }
}

impl Drop for SignalFlags {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            low_level::unregister(registration);
        }
    }
}

#[derive(Debug)]
pub(crate) struct TerminalController {
    tty: File,
    shell_pgid: Pid,
    state: Cell<TerminalState>,
    saved_modes: RefCell<Option<Termios>>,
    signals: SignalFlags,
}

impl TerminalController {
    pub(crate) fn open() -> io::Result<Option<Self>> {
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .or_else(|_| OpenOptions::new().read(true).write(true).open("/dev/stdin"))?;
        let shell_pgid = getpgrp();
        let foreground = match tcgetpgrp(&tty) {
            Ok(foreground) => foreground,
            Err(Errno::ENOTTY) => return Ok(None),
            Err(error) => return Err(io::Error::other(error)),
        };
        if foreground != shell_pgid {
            return Err(io::Error::other(format!(
                "Josh process group {shell_pgid} does not own the interactive terminal (owner is {foreground})"
            )));
        }
        tcgetattr(&tty).map_err(io::Error::other)?;
        Ok(Some(Self {
            tty,
            shell_pgid,
            state: Cell::new(TerminalState::Shell),
            saved_modes: RefCell::new(None),
            signals: SignalFlags::install()?,
        }))
    }

    pub(crate) fn handoff(&self, pgid: Pid) -> io::Result<ForegroundGuard<'_>> {
        if self.state.get() != TerminalState::Shell {
            return Err(io::Error::other(
                "cannot start a foreground pipeline while another owns the terminal",
            ));
        }
        let modes = tcgetattr(&self.tty).map_err(io::Error::other)?;
        self.signals.clear();
        *self.saved_modes.borrow_mut() = Some(modes);
        self.state.set(TerminalState::Foreground { pgid });
        if let Err(error) = set_foreground(&self.tty, pgid) {
            let _ = self.restore();
            return Err(error);
        }
        Ok(ForegroundGuard {
            controller: self,
            active: true,
        })
    }

    pub(crate) fn forward_pending(&self, pgid: Pid) {
        if self.state.get() == (TerminalState::Foreground { pgid }) {
            self.signals.forward_pending(pgid);
        }
    }

    fn restore(&self) -> io::Result<()> {
        if self.state.get() == TerminalState::Shell {
            return Ok(());
        }
        set_foreground(&self.tty, self.shell_pgid)?;
        if let Some(modes) = self.saved_modes.borrow_mut().take() {
            tcsetattr(&self.tty, SetArg::TCSADRAIN, &modes).map_err(io::Error::other)?;
        }
        self.state.set(TerminalState::Shell);
        self.signals.clear();
        Ok(())
    }
}

impl Drop for TerminalController {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub(crate) struct ForegroundGuard<'a> {
    controller: &'a TerminalController,
    active: bool,
}

impl ForegroundGuard<'_> {
    pub(crate) fn restore(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.controller.restore()?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.controller.restore();
        }
    }
}

fn set_foreground(tty: &File, pgid: Pid) -> io::Result<()> {
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGTTOU);
    let previous = blocked
        .thread_swap_mask(SigmaskHow::SIG_BLOCK)
        .map_err(io::Error::other)?;
    let result = loop {
        match tcsetpgrp(tty, pgid) {
            Ok(()) => break Ok(()),
            Err(Errno::EINTR) => {}
            Err(error) => break Err(io::Error::other(error)),
        }
    };
    let mask_result = previous.thread_set_mask().map_err(io::Error::other);
    result.and(mask_result)
}
