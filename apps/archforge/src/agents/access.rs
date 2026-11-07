//! Agent access: the control channel's requests on their way to the window, and Chat's own opening
//! of the channel, made only while a conversation holds it.
//!
//! Every opening (the person's `--control` port and each of Chat's) answers through one gated
//! handler made for it ([`handler`]): a request goes to the window with its token's role and
//! liveness, the window is woken, and the answer is awaited in short slices, so a request whose
//! token is revoked while it waits is given up at once and never runs. The window refuses what a
//! read-only token may not do (`archforge_ui_egui::control::answer_request`).
//!
//! Chat's opening ([`Access`]) is made when the first conversation is granted access, on a free
//! loopback port, and only while Let Chat use the model is on; each conversation gets its own token
//! in its own file, read-only unless the conversation allows changes; the opening closes with the
//! last conversation's grant, at once when the opt-in turns off, and on quit.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use archforge_control::{Call, Grant, Handler, ListenerWatch, Role, Server};
use archforge_ui_egui::chat::{EventSink, HostEvent};
use archforge_ui_egui::control::{ControlRequest, WAITING, abandon};

/// How often a request waiting for the window looks whether its token is still live.
const SLICE: Duration = Duration::from_millis(50);

/// The window's side of the channel: the one queue every opening posts its requests to, and the
/// window's context, once it exists, to wake it.
#[derive(Clone)]
pub struct Window {
    requests: Sender<ControlRequest>,
    context: Arc<Mutex<Option<egui::Context>>>,
}

impl Window {
    /// A window side and the queue's receiving end, which the shell drains once per frame.
    pub fn new() -> (Window, Receiver<ControlRequest>) {
        let (requests, receiver) = std::sync::mpsc::channel();
        (Window { requests, context: Arc::new(Mutex::new(None)) }, receiver)
    }

    /// Gives the window's context, so a request wakes the window.
    pub fn attach(&self, ctx: egui::Context) {
        *self.context.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ctx);
    }

    /// Wakes the window, when it exists.
    fn wake(&self) {
        if let Some(ctx) = self.context.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
            ctx.request_repaint();
        }
    }
}

/// The gated handler of one opening: each request goes to the window with its token's role and
/// liveness, and its answer is awaited up to the channel's timeout. A request whose token is
/// revoked, or that the window has not taken in time, is given up and never runs; one already
/// running gets a few seconds more, then the truth that it may have been applied.
pub fn handler(window: Window) -> Handler {
    Arc::new(move |call: Call<'_>| {
        let (reply, answer) = std::sync::mpsc::channel();
        let state = Arc::new(AtomicU8::new(WAITING));
        let request = ControlRequest {
            method: call.method.to_string(),
            params: call.params,
            origin: call.origin.map(str::to_string),
            reply,
            state: state.clone(),
            read_only: call.role == Role::ReadOnly,
            live: call.live.clone(),
        };
        window.requests.send(request).map_err(|_| "Archforge is closing".to_string())?;
        window.wake();
        let deadline = Instant::now() + archforge_control::TIMEOUT;
        loop {
            match answer.recv_timeout(SLICE) {
                Ok(answer) => return answer,
                Err(RecvTimeoutError::Disconnected) => return Err("Archforge is closing".into()),
                Err(RecvTimeoutError::Timeout) => {}
            }
            let revoked = !call.live.load(Ordering::SeqCst);
            if revoked || Instant::now() >= deadline {
                // Given up before the window took it: it never runs.
                if abandon(&state) {
                    return Err(if revoked {
                        archforge_control::REVOKED.to_string()
                    } else {
                        "Archforge did not answer in time (its window may be busy or hidden); the request was not run".into()
                    });
                }
                return answer
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap_or_else(|_| Err("Archforge did not answer in time; the request may have been applied".into()));
            }
        }
    })
}

/// The person's own access for this start (`--control <port>`): listens on `port` of the loopback
/// interface with one full-access token, its requests answered by the window.
pub fn start_control(port: u16, settings: &Path, window: &Window) -> Result<Server, String> {
    Server::start(port, settings, handler(window.clone()))
}

/// Chat's agent access: the opt-in, and the opening while a conversation holds it.
pub struct Access {
    settings: PathBuf,
    window: Window,
    /// Let Chat use the model.
    on: bool,
    server: Option<Server>,
    /// The listening socket of the last opening closed, watched after it is gone.
    closed: Option<ListenerWatch>,
}

impl Access {
    /// Access off, with no opening, its token files to go in `settings`.
    pub fn new(settings: &Path, window: Window) -> Access {
        Access { settings: settings.to_path_buf(), window, on: false, server: None, closed: None }
    }

    /// A token for one conversation, read-only unless `allow_changes`, the opening made when none
    /// is open (then the window is told agent access is held). `None` while the opt-in is off.
    pub fn grant(&mut self, allow_changes: bool, sink: Option<&EventSink>) -> Result<Option<Grant>, String> {
        if !self.on {
            return Ok(None);
        }
        let opened = self.server.is_none();
        if opened {
            self.server = Some(Server::open(&self.settings, handler(self.window.clone()))?);
        }
        let server = self.server.as_ref().ok_or("the control channel did not open")?;
        let grant = match server.grant(if allow_changes { Role::Full } else { Role::ReadOnly }) {
            Ok(g) => g,
            Err(e) => {
                // An opening made for this grant alone does not stay open without it.
                self.released(sink);
                return Err(e);
            }
        };
        if opened && let Some(sink) = sink {
            sink.send(HostEvent::Access { held: true });
        }
        Ok(Some(grant))
    }

    /// After a grant was dropped: the opening closes when no conversation holds a token any more.
    pub fn released(&mut self, sink: Option<&EventSink>) {
        if self.server.as_ref().is_some_and(|s| s.grants() == 0) {
            self.close(sink);
        }
    }

    /// Whether an opening of Chat's still listens: the one open, or the last one closed while its
    /// listening socket is not closed yet. The tests judge by it, never by the port, which anything
    /// may bind once it is free (AR-229).
    #[cfg(test)]
    pub fn listening(&self) -> bool {
        self.server.is_some() || self.closed.as_ref().is_some_and(|w| !w.closed())
    }

    /// Turns the opt-in on or off; off closes the opening at once.
    pub fn set(&mut self, on: bool, sink: Option<&EventSink>) {
        self.on = on;
        if !on {
            self.close(sink);
        }
    }

    /// Closes the opening, when one is open: every token is revoked, held connections are
    /// answered "revoked" and closed, its threads are joined, and the window is told. Its listening
    /// socket is watched after (a listener left open is said in the log).
    pub fn close(&mut self, sink: Option<&EventSink>) {
        if let Some(server) = self.server.take() {
            let port = server.port();
            let watch = server.listener_watch();
            drop(server);
            // Its accepting thread is joined in the drop when it could be woken; one that could not
            // still listens, and is said so.
            if !watch.closed() {
                log::warn!("agent access closed, but its listener on 127.0.0.1:{port} is still open");
            }
            self.closed = Some(watch);
            if let Some(sink) = sink {
                sink.send(HostEvent::Access { held: false });
            }
        }
    }
}
