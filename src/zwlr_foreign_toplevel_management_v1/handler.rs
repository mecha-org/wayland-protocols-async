use std::{sync::Mutex};
use slotmap::{new_key_type, SlotMap};
use tokio::{sync::{mpsc::{Sender, Receiver}, oneshot}, io::unix::AsyncFd};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch, event_created_child, Proxy};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{zwlr_foreign_toplevel_manager_v1::{ZwlrForeignToplevelManagerV1, self}, zwlr_foreign_toplevel_handle_v1::{ZwlrForeignToplevelHandleV1, self}};

use super::errors::{ToplevelHandlerError, ToplevelHandlerErrorCodes};

new_key_type! { pub struct ToplevelKey; }

#[derive(Debug)]
pub enum ToplevelMessage {
    GetToplevels { reply_to: oneshot::Sender<Vec<ToplevelKey>> },
    GetToplevelMeta { key: ToplevelKey, reply_to: oneshot::Sender<Option<ToplevelMeta>> },
    Activate { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    SetFullscreen { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    SetMaximize { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    SetMinimize { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    UnsetFullscreen { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    UnsetMaximize { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    UnsetMinimize { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
    Close { key: ToplevelKey, reply_to: oneshot::Sender<Result<bool, ToplevelHandlerError>> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToplevelWState {
    Maximized = 0,
    Minimized = 1,
    Activated = 2,
    Fullscreen = 3
}

fn map_state_to_wstate(state: Vec<u8>) -> Vec<ToplevelWState> {
    let mut wstate: Vec<ToplevelWState> = vec![];
    let mut state_iter = state
    .chunks_exact(4)
    .map(|b| u32::from_ne_bytes(b.try_into().unwrap()));

    let is_maximized = state_iter.clone().any(|s| s == zwlr_foreign_toplevel_handle_v1::State::Maximized as u32);
    let is_minimized = state_iter.clone().any(|s| s == zwlr_foreign_toplevel_handle_v1::State::Minimized as u32);
    let is_activated = state_iter.clone().any(|s| s == zwlr_foreign_toplevel_handle_v1::State::Activated as u32);
    let is_fullscreen = state_iter.clone().any(|s| s == zwlr_foreign_toplevel_handle_v1::State::Fullscreen as u32);

    if is_maximized {
        wstate.push(ToplevelWState::Maximized);
    }
    
    if is_minimized {
        wstate.push(ToplevelWState::Minimized);
    }

    if is_activated {
        wstate.push(ToplevelWState::Activated);
    }

    if is_fullscreen {
        wstate.push(ToplevelWState::Fullscreen);
    }

    println!("base state {:?} {:?} {:?}", state, wstate, is_activated);

    wstate
}

#[derive(Debug)]
pub enum ToplevelEvent {
    Created { key: ToplevelKey },
    Title { key: ToplevelKey, title: String },
    AppId { key: ToplevelKey, app_id: String },
    Done { key: ToplevelKey, title: String, app_id: String, state: Option<Vec<ToplevelWState>> },
    State { key: ToplevelKey, state: Vec<ToplevelWState> },
    Closed { key: ToplevelKey },
    OutputEnter { key: ToplevelKey, output: WlOutput },
    OutputLeave { key: ToplevelKey, output: WlOutput },
    Parent { key: ToplevelKey, parent: Option<ZwlrForeignToplevelHandleV1> },
}

#[derive(Debug, Clone)]
pub struct Toplevel {
    pub title: String,
    pub app_id: String,
    pub state: Option<Vec<ToplevelWState>>,
    _handle: ZwlrForeignToplevelHandleV1,
}

#[derive(Debug, Clone)]
pub struct ToplevelMeta {
    app_id: String,
    title: String,
    state: Option<Vec<ToplevelWState>>,
}

impl Toplevel {
    fn new(handle: ZwlrForeignToplevelHandleV1) -> Self {
        Toplevel {
            title: "".to_string(),
            app_id: "".to_string(),  // default
            state: None,
            _handle: handle,
        }
    }
}


pub struct ToplevelState {
    event_tx: Sender<ToplevelEvent>,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
    toplevels: SlotMap<ToplevelKey, Toplevel>,
}

pub struct ToplevelHandler {
    event_queue: EventQueue<ToplevelState>,
    state: ToplevelState,
}

impl ToplevelHandler {
    pub fn new(event_tx: Sender<ToplevelEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<ToplevelState>(&conn).unwrap();
        let qh = event_queue.handle();

        let _toplevel_manager = globals
            .bind::<ZwlrForeignToplevelManagerV1, _, _>(&qh, core::ops::RangeInclusive::new(3, 3), ())
            .map_err(|_| "compositor does not implement foreign toplevel manager (v3).").unwrap();

        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();

        let mut state = ToplevelState {
            event_tx,
            seat: Some(seat),
            output: Some(output),
            toplevels: SlotMap::with_key(),
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        ToplevelHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<ToplevelMessage>) {
        let event_queue = &mut self.event_queue;
        let mut toplevel_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut toplevel_state).unwrap();
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
                            event_queue.dispatch_pending(&mut toplevel_state).unwrap();
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
                        ToplevelMessage::GetToplevels { reply_to } => {
                            let _ = reply_to.send(toplevel_state.toplevels.keys().collect());
                        },
                        ToplevelMessage::GetToplevelMeta { key, reply_to } => {
                            let toplevel_meta = match toplevel_state.toplevels.get(key) {
                                Some(t) => Some(ToplevelMeta {
                                    app_id: t.app_id.clone(),
                                    title: t.title.clone(),
                                    state: t.state.clone(),
                                }),
                                None => None,
                            };
                            let _ = reply_to.send(toplevel_meta);
                        },
                        ToplevelMessage::Activate { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    if toplevel_state.seat.is_some() {
                                        let seat = toplevel_state.seat.as_ref().unwrap();
                                        t._handle.activate(&seat);
                                    }
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::SetFullscreen { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let output = toplevel_state.output.clone();
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.set_fullscreen(output.as_ref());
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::SetMaximize { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.set_maximized();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::SetMinimize { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.set_minimized();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::UnsetFullscreen { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.unset_fullscreen();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::UnsetMaximize { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.unset_maximized();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::UnsetMinimize { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.unset_minimized();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                        ToplevelMessage::Close { key, reply_to } => {
                            let toplevel = toplevel_state.toplevels.get_mut(key);
                            let result = match toplevel {
                                Some(t) => {
                                    t._handle.close();
                                    Ok(true)
                                },
                                None => {
                                    Err(ToplevelHandlerError::new(
                                        ToplevelHandlerErrorCodes::ToplevelNotFound,
                                        format!("toplevel not found")
                                    ))
                                },
                            };
                            let _ = reply_to.send(result);
                        },
                    }
                }
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl ToplevelState {
    fn dispatch_event(&self, event: ToplevelEvent) {
        let tx = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for ToplevelState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<ToplevelState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), ToplevelState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<ToplevelState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
        }
    }
}

impl Dispatch<WlOutput, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<ToplevelState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}


impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                let toplevel_data = Toplevel::new(toplevel.clone());

                // insert to slotmap and get toplevel key
                let toplevel_key = state.toplevels.insert(toplevel_data);

                // Add user data as mutex of the toplevel_key
                let user_data: &Mutex<Option<ToplevelKey>> = toplevel.data().unwrap();
                *user_data.lock().unwrap() = Some(toplevel_key);

                // toplevel.set_fullscreen(None);

                // send event
                state.dispatch_event(ToplevelEvent::Created { key: toplevel_key });
            },
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                // TODO: clear the states for toplevel and manager and send event for Finished
            }
            _ => (),
        }
    }

    event_created_child!(Self, ZwlrForeignToplevelHandleV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, Mutex::new(None))
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, Mutex<Option<ToplevelKey>>> for ToplevelState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as Proxy>::Event,
        key: &Mutex<Option<ToplevelKey>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                let toplevel_key = key.lock().unwrap().unwrap();

                let toplevel = state
                    .toplevels
                    .get_mut(toplevel_key)
                    .unwrap();
                toplevel.title = title.clone();

                state.dispatch_event(ToplevelEvent::Title { key: toplevel_key, title });
            },
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                let toplevel_key = key.lock().unwrap().unwrap();
                let toplevel = state
                    .toplevels
                    .get_mut(toplevel_key)
                    .unwrap();
                toplevel.app_id = app_id.clone();

                state.dispatch_event(ToplevelEvent::AppId { key: toplevel_key, app_id });
            },
            zwlr_foreign_toplevel_handle_v1::Event::State { state: toplevel_state } => {
                let toplevel_key = key.lock().unwrap().unwrap();
                let toplevel = state
                    .toplevels
                    .get_mut(toplevel_key)
                    .unwrap();
                let toplevel_state = map_state_to_wstate(toplevel_state);
                toplevel.state = Some(toplevel_state.clone());

                state.dispatch_event(ToplevelEvent::State { key: toplevel_key, state: toplevel_state.clone() });
            },
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                let toplevel_key = key.lock().unwrap().unwrap();
                let toplevel = state
                    .toplevels
                    .get_mut(toplevel_key)
                    .unwrap();
                let title = toplevel.title.clone();
                let app_id = toplevel.app_id.clone();
                let toplevel_state = toplevel.state.clone();

                state.dispatch_event(ToplevelEvent::Done { key: toplevel_key, title, app_id, state: toplevel_state });
            },
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                let toplevel_key = key.lock().unwrap().unwrap();
                let toplevel = state
                    .toplevels
                    .get_mut(toplevel_key)
                    .unwrap();

                // removing from state, you cannot use the toplevel after that
                state.toplevels.remove(toplevel_key);
                
                state.dispatch_event(ToplevelEvent::Closed { key: toplevel_key });
            },
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { output } => {
                let toplevel_key = key.lock().unwrap().unwrap();
                state.dispatch_event(ToplevelEvent::OutputEnter { key: toplevel_key, output });
            },
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { output } => {
                let toplevel_key = key.lock().unwrap().unwrap();
                state.dispatch_event(ToplevelEvent::OutputLeave { key: toplevel_key, output });
            },
            zwlr_foreign_toplevel_handle_v1::Event::Parent { parent } => {
                let toplevel_key = key.lock().unwrap().unwrap();
                state.dispatch_event(ToplevelEvent::Parent { key: toplevel_key, parent });
            },
            _ => {},
        }
    }
}
