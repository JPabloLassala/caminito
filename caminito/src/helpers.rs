//! The supervisor for Caminito's helper processes (15): the palette helper and the locker
//! (126).
//!
//! A helper is spawned with its stdin and stdout piped to Caminito. Its stdout is a calloop
//! fd source, and its stderr goes to the journal under the helper's own identifier, or to
//! Caminito's stderr with that identifier as a prefix outside a session. Exits arrive
//! through a `SIGCHLD` source: signal-hook's pipe, which, unlike a signalfd, doesn't need
//! the signal blocked in every thread. A helper that exits is restarted with backoff
//! ([`RestartPolicy`]), until it has exited too often, when it's left stopped with one
//! log line, or until it's finished ([`Helper::finish`]).
//!
//! Xwayland, the third helper, is spawned by Smithay, so only its stderr goes through here
//! ([`stderr_to`]), and its restarts are counted with [`Restarts`] (42).

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction, RegistrationToken};

/// What a helper runs.
#[derive(Debug, Clone)]
pub struct HelperCommand {
    /// The name its stderr is logged under.
    identifier: String,
    program: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

impl HelperCommand {
    pub fn new(identifier: impl Into<String>, program: impl Into<PathBuf>) -> Self {
        HelperCommand {
            identifier: identifier.into(),
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// Adds arguments for the helper, after its program.
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Sets an environment variable for the helper, on top of Caminito's own.
    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }
}

/// Where a helper's stderr goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StderrTarget {
    /// The journal, under the helper's identifier (in a session, 14).
    Journal,
    /// Caminito's stderr, each line prefixed with the helper's identifier.
    Prefixed,
}

/// What happened to a helper, for the code that uses it.
#[derive(Debug)]
pub enum HelperEvent<'a> {
    /// It was spawned, and its stdin is open.
    Started,
    /// The next bytes of its stdout.
    Stdout(&'a [u8]),
    /// It exited, or couldn't be spawned. Its stdout has been read to the end.
    Exited,
    /// It exited too often and won't be started again.
    GaveUp,
}

/// One supervised helper, which lives in the loop's data `D`.
pub struct Helper<D: 'static> {
    command: HelperCommand,
    policy: RestartPolicy,
    stderr: StderrTarget,
    handle: LoopHandle<'static, D>,
    /// Finds this helper in the loop's data.
    access: fn(&mut D) -> &mut Helper<D>,
    on_event: fn(&mut D, HelperEvent<'_>),
    restarts: Restarts,
    running: Option<Running>,
    sigchld: Option<(signal_hook::SigId, RegistrationToken)>,
    restart_timer: Option<RegistrationToken>,
    /// Whether it's finished: it's not started again.
    finished: bool,
}

/// A spawned helper.
struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: Rc<File>,
    sources: Vec<RegistrationToken>,
}

impl<D: 'static> Helper<D> {
    /// A helper that isn't started yet.
    pub fn new(
        handle: LoopHandle<'static, D>,
        command: HelperCommand,
        policy: RestartPolicy,
        stderr: StderrTarget,
        access: fn(&mut D) -> &mut Helper<D>,
        on_event: fn(&mut D, HelperEvent<'_>),
    ) -> Self {
        Helper {
            command,
            policy,
            stderr,
            handle,
            access,
            on_event,
            restarts: Restarts::default(),
            running: None,
            sigchld: None,
            restart_timer: None,
            finished: false,
        }
    }

    /// Spawns the helper when the loop next runs. Once started, it's supervised until it's
    /// dropped, so starting it again does nothing.
    pub fn start(&mut self) {
        if self.sigchld.is_some() {
            return;
        }
        match self.watch_exits() {
            Ok(sigchld) => self.sigchld = Some(sigchld),
            Err(err) => {
                tracing::error!("{}: can't watch for exits: {err}", self.command.identifier);
                return;
            }
        }
        let access = self.access;
        self.handle
            .insert_idle(move |data| Self::spawn(data, access));
    }

    /// Stops the helper for good, as Caminito tears down (08): it's sent SIGTERM, and it's
    /// neither watched nor started again. Caminito doesn't wait for it to exit.
    pub fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            for token in running.sources {
                self.handle.remove(token);
            }
            let pid = rustix::process::Pid::from_child(&running.child);
            if let Err(err) = rustix::process::kill_process(pid, rustix::process::Signal::TERM) {
                tracing::warn!("{}: can't send it SIGTERM: {err}", self.command.identifier);
            }
        }
        if let Some((id, token)) = self.sigchld.take() {
            signal_hook::low_level::unregister(id);
            self.handle.remove(token);
        }
        if let Some(token) = self.restart_timer.take() {
            self.handle.remove(token);
        }
    }

    /// Lets the helper be: the one that runs is left running and reaped when it exits, and
    /// none is started again. It's how a helper whose work is done ends, as the locker after
    /// the unlock (126).
    pub fn finish(&mut self) {
        self.finished = true;
        if let Some(token) = self.restart_timer.take() {
            self.handle.remove(token);
        }
    }

    /// The process of the helper that runs, if one does.
    pub fn process(&self) -> Option<u32> {
        self.running.as_ref().map(|running| running.child.id())
    }

    /// Writes to the helper's stdin, which never blocks: a full pipe is an error. A write of
    /// at most `PIPE_BUF` (4096) bytes is all or nothing, so a request that size is never
    /// left half written.
    pub fn write_stdin(&mut self, bytes: &[u8]) -> io::Result<()> {
        let Some(running) = &mut self.running else {
            return Err(io::ErrorKind::NotConnected.into());
        };
        running.stdin.write_all(bytes)
    }

    fn watch_exits(&self) -> io::Result<(signal_hook::SigId, RegistrationToken)> {
        let (read, write) = UnixStream::pair()?;
        read.set_nonblocking(true)?;
        write.set_nonblocking(true)?;
        let id = signal_hook::low_level::pipe::register(signal_hook::consts::SIGCHLD, write)?;
        let access = self.access;
        let source = Generic::new(read, Interest::READ, Mode::Level);
        let token = self
            .handle
            .insert_source(source, move |_, read, data| {
                let mut buffer = [0; 64];
                while matches!((&**read).read(&mut buffer), Ok(n) if n > 0) {}
                Self::reap(data, access);
                Ok(PostAction::Continue)
            })
            .map_err(|err| io::Error::other(err.error))?;
        Ok((id, token))
    }

    fn spawn(data: &mut D, access: fn(&mut D) -> &mut Helper<D>) {
        let helper = access(data);
        helper.restart_timer = None;
        // It was stopped or finished between `start` and the loop's next run, or the loop
        // data has another helper there now, which runs already.
        if helper.sigchld.is_none() || helper.finished || helper.running.is_some() {
            return;
        }
        match helper.try_spawn() {
            Ok(running) => {
                helper.running = Some(running);
                let on_event = helper.on_event;
                on_event(data, HelperEvent::Started);
            }
            Err(err) => {
                tracing::warn!(
                    "{}: can't start {}: {err}",
                    helper.command.identifier,
                    helper.command.program.display()
                );
                Self::exited(data, access);
            }
        }
    }

    fn try_spawn(&mut self) -> io::Result<Running> {
        let stderr = stderr_to(&self.handle, &self.command.identifier, self.stderr)?;
        let mut child = Command::new(&self.command.program)
            .args(&self.command.args)
            .envs(self.command.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()?;
        let mut sources = Vec::new();
        match self.supervise(&mut child, &mut sources) {
            Ok((stdin, stdout)) => Ok(Running {
                child,
                stdin,
                stdout,
                sources,
            }),
            // It's never left running unsupervised.
            Err(err) => {
                for token in sources {
                    self.handle.remove(token);
                }
                let _ = child.kill();
                let _ = child.wait();
                Err(err)
            }
        }
    }

    /// Adds the source for a spawned child's stdout.
    fn supervise(
        &self,
        child: &mut Child,
        sources: &mut Vec<RegistrationToken>,
    ) -> io::Result<(ChildStdin, Rc<File>)> {
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = OwnedFd::from(child.stdout.take().expect("stdout is piped"));
        set_nonblocking(&stdin)?;
        set_nonblocking(&stdout)?;
        let stdout = Rc::new(File::from(stdout));

        let access = self.access;
        let on_event = self.on_event;
        let source = Generic::new(Rc::clone(&stdout), Interest::READ, Mode::Level);
        let token = self.handle.insert_source(source, move |_, stdout, data| {
            let mut buffer = [0; 4096];
            match (&***stdout).read(&mut buffer) {
                Ok(0) => Ok(PostAction::Remove),
                Ok(n) => {
                    on_event(data, HelperEvent::Stdout(&buffer[..n]));
                    Ok(PostAction::Continue)
                }
                Err(err) if would_block(&err) => Ok(PostAction::Continue),
                Err(err) => {
                    let identifier = &access(data).command.identifier;
                    tracing::warn!("{identifier}: can't read its stdout: {err}");
                    Ok(PostAction::Remove)
                }
            }
        });
        sources.push(token.map_err(|err| io::Error::other(err.error))?);
        Ok((stdin, stdout))
    }

    /// Handles a `SIGCHLD`, which may be for another child.
    fn reap(data: &mut D, access: fn(&mut D) -> &mut Helper<D>) {
        let helper = access(data);
        let Some(running) = &mut helper.running else {
            return;
        };
        let status = match running.child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("{}: can't wait for it: {err}", helper.command.identifier);
                return;
            }
        };
        let running = helper.running.take().expect("it was running");
        for token in running.sources {
            helper.handle.remove(token);
        }
        tracing::warn!("{} exited: {status}", helper.command.identifier);

        // What it wrote before it exited, which its source may not have read yet.
        let mut rest = Vec::new();
        let _ = (&*running.stdout).read_to_end(&mut rest);
        let on_event = helper.on_event;
        if !rest.is_empty() {
            on_event(data, HelperEvent::Stdout(&rest));
        }
        Self::exited(data, access);
    }

    fn exited(data: &mut D, access: fn(&mut D) -> &mut Helper<D>) {
        let helper = access(data);
        let on_event = helper.on_event;
        if helper.finished {
            on_event(data, HelperEvent::Exited);
            return;
        }
        match helper.restarts.exited(Instant::now(), &helper.policy) {
            Some(delay) => {
                let timer =
                    helper
                        .handle
                        .insert_source(Timer::from_duration(delay), move |_, _, data| {
                            Self::spawn(data, access);
                            TimeoutAction::Drop
                        });
                match timer {
                    Ok(token) => helper.restart_timer = Some(token),
                    Err(err) => tracing::error!(
                        "{}: can't schedule its restart: {}",
                        helper.command.identifier,
                        err.error
                    ),
                }
                on_event(data, HelperEvent::Exited);
            }
            None => {
                tracing::error!(
                    "{} exited {} times in {:?}, so it won't be started again",
                    helper.command.identifier,
                    helper.policy.give_up_after,
                    helper.policy.window
                );
                on_event(data, HelperEvent::Exited);
                on_event(data, HelperEvent::GaveUp);
            }
        }
    }
}

impl<D: 'static> Drop for Helper<D> {
    fn drop(&mut self) {
        if let Some(mut running) = self.running.take() {
            for token in running.sources {
                self.handle.remove(token);
            }
            let _ = running.child.kill();
            let _ = running.child.wait();
        }
        if let Some((id, token)) = self.sigchld.take() {
            signal_hook::low_level::unregister(id);
            self.handle.remove(token);
        }
        if let Some(token) = self.restart_timer.take() {
            self.handle.remove(token);
        }
    }
}

/// The stderr to spawn a helper with, for `target`: a journal stream, or a pipe whose lines
/// a loop source prefixes with `identifier` onto Caminito's stderr until the helper closes it.
/// A helper that Caminito doesn't spawn itself, such as Xwayland, is given it too (15).
pub fn stderr_to<D: 'static>(
    handle: &LoopHandle<'static, D>,
    identifier: &str,
    target: StderrTarget,
) -> io::Result<Stdio> {
    if target == StderrTarget::Journal {
        match journal_stream(identifier) {
            Ok(stream) => return Ok(stream.into()),
            Err(err) => tracing::warn!(
                "{identifier}: can't log to the journal, so its stderr goes to Caminito's: {err}"
            ),
        }
    }
    let (reader, writer) = io::pipe()?;
    let reader = File::from(OwnedFd::from(reader));
    set_nonblocking(&reader)?;
    let identifier = identifier.to_owned();
    let mut partial = Vec::new();
    let source = Generic::new(reader, Interest::READ, Mode::Level);
    handle
        .insert_source(source, move |_, stderr, _| {
            Ok(prefix_lines(&identifier, stderr, &mut partial))
        })
        .map_err(|err| io::Error::other(err.error))?;
    Ok(writer.into())
}

/// Reads what's there of a helper's stderr and writes it to Caminito's, a line at a time.
/// A line cut by the read waits in `partial` for the rest.
fn prefix_lines(identifier: &str, stderr: &File, partial: &mut Vec<u8>) -> PostAction {
    let mut buffer = [0; 4096];
    let n = match (&*stderr).read(&mut buffer) {
        Ok(n) => n,
        Err(err) if would_block(&err) => return PostAction::Continue,
        Err(_) => 0,
    };
    partial.extend_from_slice(&buffer[..n]);
    let complete = if n == 0 {
        partial.len()
    } else {
        partial
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1)
    };
    for line in String::from_utf8_lossy(&partial[..complete]).lines() {
        eprintln!("{identifier}: {line}");
    }
    partial.drain(..complete);
    if n == 0 {
        PostAction::Remove
    } else {
        PostAction::Continue
    }
}

/// A stream to journald under this identifier, as `sd_journal_stream_fd` makes one
/// (`systemd-cat`).
fn journal_stream(identifier: &str) -> io::Result<OwnedFd> {
    let mut stream = UnixStream::connect("/run/systemd/journal/stdout")?;
    stream.shutdown(std::net::Shutdown::Read)?;
    // Identifier, unit, priority (info), level prefix, and forwarding to syslog, kmsg and
    // the console, all off.
    stream.write_all(format!("{identifier}\n\n6\n0\n0\n0\n0\n").as_bytes())?;
    Ok(stream.into())
}

fn set_nonblocking(fd: impl std::os::fd::AsFd) -> io::Result<()> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    let flags = fcntl_getfl(&fd)?;
    fcntl_setfl(&fd, flags | OFlags::NONBLOCK)?;
    Ok(())
}

fn would_block(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// When a helper that exited is started again.
#[derive(Debug, Clone, Copy)]
pub struct RestartPolicy {
    /// The delay after an exit with no other recent ones. It doubles with each further
    /// exit within `window`.
    pub first_delay: Duration,
    pub max_delay: Duration,
    /// This many exits within `window` and the helper is left stopped.
    pub give_up_after: usize,
    pub window: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy {
            first_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            give_up_after: 5,
            window: Duration::from_secs(60),
        }
    }
}

/// A helper's recent exits.
#[derive(Debug, Default)]
pub(crate) struct Restarts {
    exits: VecDeque<Instant>,
}

impl Restarts {
    /// Records an exit, and returns how long to wait before starting the helper again, or
    /// None to give up.
    pub(crate) fn exited(&mut self, now: Instant, policy: &RestartPolicy) -> Option<Duration> {
        while self
            .exits
            .front()
            .is_some_and(|&exit| now.duration_since(exit) >= policy.window)
        {
            self.exits.pop_front();
        }
        self.exits.push_back(now);
        let recent = self.exits.len();
        if recent >= policy.give_up_after {
            return None;
        }
        let doublings = u32::try_from(recent - 1).unwrap_or(u32::MAX);
        let delay = policy
            .first_delay
            .checked_mul(2u32.saturating_pow(doublings))
            .unwrap_or(policy.max_delay);
        Some(delay.min(policy.max_delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RestartPolicy {
        RestartPolicy {
            first_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(500),
            give_up_after: 5,
            window: Duration::from_secs(60),
        }
    }

    #[test]
    fn exits_close_together_back_off_then_give_up() {
        let start = Instant::now();
        let mut restarts = Restarts::default();
        let delays: Vec<Option<Duration>> = (0..5)
            .map(|n| restarts.exited(start + Duration::from_secs(n), &policy()))
            .collect();
        let ms = |ms| Some(Duration::from_millis(ms));
        assert_eq!(delays, vec![ms(100), ms(200), ms(400), ms(500), None]);
    }

    #[test]
    fn exits_spread_over_more_than_the_window_never_give_up() {
        let start = Instant::now();
        let mut restarts = Restarts::default();
        for n in 0..20 {
            let delay = restarts.exited(start + Duration::from_secs(n * 16), &policy());
            assert!(delay.is_some(), "exit {n}");
        }
    }

    #[test]
    fn the_back_off_starts_again_once_old_exits_leave_the_window() {
        let start = Instant::now();
        let mut restarts = Restarts::default();
        for n in 0..4 {
            restarts.exited(start + Duration::from_secs(n), &policy());
        }
        let later = start + Duration::from_secs(120);
        assert_eq!(
            restarts.exited(later, &policy()),
            Some(Duration::from_millis(100))
        );
    }
}
