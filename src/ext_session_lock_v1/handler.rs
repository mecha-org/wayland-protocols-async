use tokio::{sync::{mpsc::{Sender, Receiver}, oneshot}, io::unix::AsyncFd};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self}, wl_compositor::{WlCompositor, self}, wl_surface::{WlSurface, self}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch, WEnum};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::{self, ExtSessionLockManagerV1}, ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1}, ext_session_lock_v1::{self, ExtSessionLockV1},
};

use super::errors::{SessionLockhandlerError, SessionLockhandlerErrorCodes};

#[derive(Debug)]
pub enum SessionLockMessage {
    Lock { wl_surface: Option<WlSurface>, reply_to: oneshot::Sender<Result<bool, SessionLockhandlerError>> },
    Unlock { reply_to: oneshot::Sender<Result<bool, SessionLockhandlerError>> },
    AckConfigure { reply_to: oneshot::Sender<Result<bool, SessionLockhandlerError>> },
}

#[derive(Debug)]
pub enum SessionLockEvent {
    Locked,
    Finished,
    Configure { serial: u32, width: u32, height: u32 },
}

pub struct SessionLockState {
    qh: QueueHandle<SessionLockState>,
    event_tx: Sender<SessionLockEvent>,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
    compositor: Option<WlCompositor>,
    surface: Option<WlSurface>,
    session_lock_manager: Option<ExtSessionLockManagerV1>,
    session_lock: Option<ExtSessionLockV1>,
    session_lock_surface: Option<ExtSessionLockSurfaceV1>,
    session_lock_configure_serial: Option<u32>
}

pub struct SessionLockHandler {
    event_queue: EventQueue<SessionLockState>,
    state: SessionLockState,
}

impl SessionLockHandler {
    pub fn new(event_tx: Sender<SessionLockEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<SessionLockState>(&conn).unwrap();
        let qh = event_queue.handle();

        let session_lock_manager = globals
            .bind::<ExtSessionLockManagerV1, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "compositor does not implement ext session lock manager (v1).").unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();
        let compositor = globals
            .bind::<WlCompositor, _, _>(&qh, core::ops::RangeInclusive::new(1, 5), ())
            .map_err(|_| "failed to retrieve the compositor from global").unwrap();
        let surface = compositor.create_surface(&qh, ());

        let mut state = SessionLockState {
            qh,
            event_tx,
            seat: Some(seat),
            output: Some(output),
            compositor: Some(compositor),
            surface: Some(surface),
            session_lock_manager: Some(session_lock_manager),
            session_lock: None,
            session_lock_surface: None,
            session_lock_configure_serial: None,
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        SessionLockHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<SessionLockMessage>) {
        let event_queue = &mut self.event_queue;
        let mut session_lock_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut session_lock_state).unwrap();
            let read_guard = event_queue.prepare_read().unwrap();
            let fd = read_guard.connection_fd();
            let async_fd = AsyncFd::new(fd).unwrap();
    
            tokio::select! {
                async_guard = async_fd.readable() => {
                    async_guard.unwrap().clear_ready();
                    // Drop the async_fd since it's holding a reference to the read_guard,
                    // which is dropped on read. We don't need to read from it anyways.
                    std::mem::drop(async_fd);
                    // This should not block because we already ensured readiness
                    let event = read_guard.read();
                    match event {
                        // There are events but another thread processed them, we don't need to dispatch
                        Ok(0) => {},
                        // We have some events
                        Ok(_) => {
                            event_queue.dispatch_pending(&mut session_lock_state).unwrap();
                        },
                        // No events to receive
                        Err(_) => {}
                        // Err(e) => eprintln!("error reading event {}", e),
                    }
                },
                msg = msg_rx.recv() => {
                    if msg.is_none() {
                        continue;
                    }
                    match msg.unwrap() {
                        SessionLockMessage::Lock { wl_surface, reply_to } => {
                            let res = session_lock_state.lock(wl_surface);
                            let _ = reply_to.send(res);
                        },
                        SessionLockMessage::Unlock { reply_to } => {
                            let res = session_lock_state.unlock();
                            let _ = reply_to.send(res);
                        },
                        SessionLockMessage::AckConfigure { reply_to } => {
                            let res = session_lock_state.ack_configure();
                            let _ = reply_to.send(res);
                        },
                    }
                }
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl SessionLockState {
    fn dispatch_event(&self, event: SessionLockEvent) {
        let tx: Sender<SessionLockEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }

    fn lock(&mut self, wl_surface: Option<WlSurface>) -> Result<bool, SessionLockhandlerError> {
        let session_lock_manager = match &self.session_lock_manager {
            Some(m) => m,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::ManagerIsNotSet,
                    format!("session lock manager uninitialized or is not set")
                ));
            },
        };
        let output = match &self.output {
            Some(o) => o,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::WLOutputIsNotSet,
                    format!("wl_output uninitialized or is not set")
                ));
            },
        };
        // use surface from argument or use internal surface
        let mut surface = match &self.surface {
            Some(o) => o,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::WLSurfaceIsNotSet,
                    format!("wl_surface uninitialized or is not set")
                ));
            },
        };
        if wl_surface.is_some() {
            surface = wl_surface.as_ref().unwrap();
        }

        let session_lock = session_lock_manager.lock(&self.qh, ());

        // set surface role as session lock surface
        session_lock.get_lock_surface(&surface, &output, &self.qh, ());

        // save to state
        self.session_lock = Some(session_lock);
        Ok(true)
    }

    fn unlock(&mut self) -> Result<bool, SessionLockhandlerError> {
        let session_lock = match &self.session_lock {
            Some(l) => l,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::SessionLockIsNotSet,
                    format!("session_lock uninitialized or is not set, did you lock the session?")
                ));
            },
        };

        // unlock session and destroy the lock
        session_lock.unlock_and_destroy();

        // remove from state
        self.session_lock = None;
        self.session_lock_surface = None;
        self.session_lock_configure_serial = None;

        Ok(true)
    }

    fn ack_configure(&self) -> Result<bool, SessionLockhandlerError> {
        let session_lock_surface = match &self.session_lock_surface {
            Some(l) => l,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::SessionLockSurfaceIsNotSet,
                    format!("session_lock_surface uninitialized or is not set, did you lock the session?")
                ));
            },
        };
        let session_lock_configure_serial = match self.session_lock_configure_serial {
            Some(l) => l,
            None => {
                return Err(SessionLockhandlerError::new(
                    SessionLockhandlerErrorCodes::SessionLockSurfaceSerialIsNotSet,
                    format!("session_lock_configure_serial uninitialized or is not set")
                ));
            },
        }; 

        // send the configuration acknowledge
        session_lock_surface.ack_configure(session_lock_configure_serial);

        Ok(true)
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for SessionLockState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<SessionLockState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), SessionLockState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<SessionLockState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
        }
    }
}

impl Dispatch<WlOutput, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<SessionLockState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}

impl Dispatch<WlCompositor, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        compositor: &WlCompositor,
        event: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<SessionLockState>,
    ) {
        state.compositor = Some(compositor.to_owned());
    }
}

impl Dispatch<WlSurface, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        surface: &WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<SessionLockState>,
    ) {
        state.surface = Some(surface.to_owned());
    }
}

impl Dispatch<ExtSessionLockManagerV1, ()> for SessionLockState {
    fn event(
        _: &mut Self,
        _: &ExtSessionLockManagerV1,
        event: <ExtSessionLockManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ExtSessionLockV1, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        _: &ExtSessionLockV1,
        event: <ExtSessionLockV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.dispatch_event(SessionLockEvent::Locked);
            },
            ext_session_lock_v1::Event::Finished => {
                state.session_lock = None;
                state.session_lock_surface = None;
                state.session_lock_configure_serial = None;

                state.dispatch_event(SessionLockEvent::Finished);
            },
            _ => {},
        }
    }
}

impl Dispatch<ExtSessionLockSurfaceV1, ()> for SessionLockState {
    fn event(
        state: &mut Self,
        surface: &ExtSessionLockSurfaceV1,
        event: <ExtSessionLockSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.session_lock_surface = Some(surface.to_owned());
        match event {
            ext_session_lock_surface_v1::Event::Configure { serial, width, height } => {
                state.session_lock_configure_serial = Some(serial);
                state.dispatch_event(SessionLockEvent::Configure { serial, width, height })
            },
            _ => todo!(),
        }
    }
}

