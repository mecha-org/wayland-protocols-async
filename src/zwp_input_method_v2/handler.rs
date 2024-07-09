use std::num::Wrapping;
use tokio::{sync::mpsc::{Sender, Receiver}, io::unix::AsyncFd};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch, WEnum, backend::protocol::WEnumError};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{ChangeCause, ContentHint, ContentPurpose};
use wayland_protocols_misc::zwp_input_method_v2::client::{zwp_input_method_manager_v2::ZwpInputMethodManagerV2, zwp_input_method_v2::{self, ZwpInputMethodV2}};

#[derive(Debug)]
pub enum InputMethodMessage {
    CommitString { text: String },
    SetPreeditString { text: String, cursor_begin: i32, cursor_end: i32 },
    DeleteSurroundingText { before_length: u32, after_length: u32 },
    Commit,
    // GetInputPopupSurface,
    // GrabKeyboard,
}

#[derive(Debug)]
pub enum InputMethodEvent {
    Activate,
    Deactivate,
    Done,
    SurroundingText { text: String, cursor: u32, anchor: u32 },
    TextChangeCause { cause: Result<ChangeCause, WEnumError> },
    ContentType { hint: Result<ContentHint, WEnumError>, purpose: Result<ContentPurpose, WEnumError>},
    Unavailable,
}

pub struct InputMethodState {
    event_tx: Sender<InputMethodEvent>,
    input_method: ZwpInputMethodV2,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
    serial: Wrapping<u32>,
}

pub struct InputMethodHandler {
    event_queue: EventQueue<InputMethodState>,
    state: InputMethodState,
}

impl InputMethodHandler {
    pub fn new(event_tx: Sender<InputMethodEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<InputMethodState>(&conn).unwrap();
        let qh = event_queue.handle();

        let input_manager = globals
            .bind::<ZwpInputMethodManagerV2, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "compositor does not implement input method (v1).").unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();

        // bind the input method
        let input_method = input_manager.get_input_method(&seat, &qh, ());

        let mut state = InputMethodState {
            event_tx,
            input_method,
            seat: Some(seat),
            output: Some(output),
            serial: Wrapping(0),
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        InputMethodHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<InputMethodMessage>) {
        let event_queue = &mut self.event_queue;
        let mut input_method_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut input_method_state).unwrap();
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
                            event_queue.dispatch_pending(&mut input_method_state).unwrap();
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
                        InputMethodMessage::CommitString{ text } => {
                            input_method_state.input_method.commit_string(text)
                        },
                        InputMethodMessage::SetPreeditString { text, cursor_begin, cursor_end } => {
                            input_method_state.input_method.set_preedit_string(text, cursor_begin, cursor_end)
                        },
                        InputMethodMessage::DeleteSurroundingText { before_length, after_length } => {
                            input_method_state.input_method.delete_surrounding_text(before_length, after_length)
                        },
                        InputMethodMessage::Commit => {
                            input_method_state.input_method.commit(input_method_state.serial.0);
                        },
                    }
                }
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl InputMethodState {
    fn dispatch_event(&self, event: InputMethodEvent) {
        let tx: Sender<InputMethodEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for InputMethodState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<InputMethodState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), InputMethodState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for InputMethodState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<InputMethodState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
        }
    }
}

impl Dispatch<WlOutput, ()> for InputMethodState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<InputMethodState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}

impl Dispatch<ZwpInputMethodManagerV2, ()> for InputMethodState {
    fn event(
        _: &mut Self,
        _: &ZwpInputMethodManagerV2,
        event: <ZwpInputMethodManagerV2 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            _ => (),
        }
    }
}


impl Dispatch<ZwpInputMethodV2, ()> for InputMethodState {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<InputMethodState>,
    ) {
        match event {
            zwp_input_method_v2::Event::Activate =>
                state.dispatch_event(InputMethodEvent::Activate),
            zwp_input_method_v2::Event::Deactivate =>
                state.dispatch_event(InputMethodEvent::Deactivate),
            zwp_input_method_v2::Event::Done => {
                state.serial += Wrapping(1u32);
                state.dispatch_event(InputMethodEvent::Done);
            }  
            zwp_input_method_v2::Event::SurroundingText { text, cursor, anchor } =>
                state.dispatch_event(InputMethodEvent::SurroundingText { text, cursor, anchor }),
            zwp_input_method_v2::Event::TextChangeCause { cause } => {
                state.dispatch_event(InputMethodEvent::TextChangeCause {
                    cause: cause.into_result()
                })
            },
            zwp_input_method_v2::Event::ContentType { hint, purpose } => {
                state.dispatch_event(InputMethodEvent::ContentType {
                    hint: hint.into_result(),
                    purpose: purpose.into_result(),
                })
            },
            zwp_input_method_v2::Event::Unavailable =>
            state.dispatch_event(InputMethodEvent::Unavailable),
            _ => {}
        }
    }
}
