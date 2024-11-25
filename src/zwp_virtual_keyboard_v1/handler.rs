use std::time::Instant;
use tokio::{
    io::unix::AsyncFd,
    sync::mpsc::{Receiver, Sender},
};
use wayland_client::{
    globals::{self, GlobalListContents},
    protocol::{
        wl_output::{self, WlOutput},
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

#[derive(Debug)]
pub enum VirtualKeyboardMessage {
    Key {
        keycode: u32,
        keymotion: KeyMotion,
    },
    SetKeymap {
        keymap_raw_fd: i32,
        keymap_size: u32,
    },
    SetModifiers { 
        depressed: u32, 
        latched: u32, 
        locked: u32
    }
}

#[derive(Debug)]
pub enum VirtualKeyboardEvent {}

#[derive(Debug, PartialEq, Eq, Clone)]
/// Enum to differentiate between a key press and a release
pub enum KeyMotion {
    Press = 1,
    Release = 0,
}

#[derive(Debug, Default,  PartialEq, Eq, Clone)]
pub struct Modifiers {
    depressed: u32,
    latched: u32,
    locked: u32,
}

pub struct VirtualKeyboardState {
    event_tx: Sender<VirtualKeyboardEvent>,
    virtual_keyboard: ZwpVirtualKeyboardV1,
    modifiers: Modifiers,
    init_time: std::time::Instant,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
}

pub struct VirtualKeyboardHandler {
    event_queue: EventQueue<VirtualKeyboardState>,
    state: VirtualKeyboardState,
}

impl VirtualKeyboardHandler {
    pub fn new(
        keymap_raw_fd: i32,
        keymap_size: u32,
        event_tx: Sender<VirtualKeyboardEvent>,
    ) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) =
            globals::registry_queue_init::<VirtualKeyboardState>(&conn).unwrap();
        let qh = event_queue.handle();

        let _virtual_keyboard_manager = globals
            .bind::<ZwpVirtualKeyboardManagerV1, _, _>(
                &qh,
                core::ops::RangeInclusive::new(1, 1),
                (),
            )
            .map_err(|_| "compositor does not implement virtual keyboard (v1).")
            .unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();

        let virtual_keyboard = _virtual_keyboard_manager.create_virtual_keyboard(&seat, &qh, ());
        virtual_keyboard.keymap(1, keymap_raw_fd, keymap_size);

        let mut state = VirtualKeyboardState {
            event_tx,
            virtual_keyboard,
            init_time: Instant::now(),
            seat: Some(seat),
            output: Some(output),
            modifiers: Modifiers::default()
        };

        event_queue.roundtrip(&mut state).unwrap();

        VirtualKeyboardHandler { event_queue, state }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<VirtualKeyboardMessage>) {
        let event_queue = &mut self.event_queue;
        let mut virtual_keyboard_state = &mut self.state;

        loop {
            // This would be required if other threads were reading from the socket.
            event_queue
                .dispatch_pending(&mut virtual_keyboard_state)
                .unwrap();
            let read_guard = event_queue.prepare_read().unwrap();
            let fd = read_guard.connection_fd();
            let async_fd = AsyncFd::new(fd).unwrap();

            tokio::select! {
                async_guard = async_fd.readable() => {
                    println!("async readable1");
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
                            event_queue.dispatch_pending(&mut virtual_keyboard_state).unwrap();
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
                        VirtualKeyboardMessage::Key { keycode, keymotion } => {
                            let _ = virtual_keyboard_state.send_key(keycode, keymotion);
                        }
                        VirtualKeyboardMessage::SetKeymap { keymap_raw_fd, keymap_size } => {
                            let _ = virtual_keyboard_state.set_keymap(keymap_raw_fd, keymap_size);
                        }
                        VirtualKeyboardMessage::SetModifiers { depressed, latched, locked } => {
                            let modifiers = Modifiers { depressed, latched, locked };
                            virtual_keyboard_state.modifiers = modifiers.clone();
                            let _ = virtual_keyboard_state.set_modifiers(modifiers);
                        }
                        ,
                    }
                }
            }

            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl VirtualKeyboardState {
    pub fn send_key(&self, keycode: u32, keymotion: KeyMotion) {
        let time = self.get_time();
        let _ = self.set_modifiers(self.modifiers.clone());
        let res = self.virtual_keyboard.key(time, keycode, keymotion as u32);
    }

    pub fn set_keymap(&self, keymap_raw_fd: i32, keymap_size: u32) {
        let _ = self.virtual_keyboard.keymap(1, keymap_raw_fd, keymap_size);
    }

    pub fn set_modifiers(&self, modifiers: Modifiers) {
        let Modifiers { depressed, latched, locked } = modifiers;
        let _ = self.virtual_keyboard.modifiers(depressed, latched, locked, 0);
    }

    fn dispatch_event(&self, event: VirtualKeyboardEvent) {
        let tx: Sender<VirtualKeyboardEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }

    fn get_time(&self) -> u32 {
        let duration = self.init_time.elapsed();
        let time = duration.as_millis();
        time.try_into().unwrap()
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<VirtualKeyboardState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), VirtualKeyboardState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for VirtualKeyboardState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<VirtualKeyboardState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
        }
    }
}

impl Dispatch<WlOutput, ()> for VirtualKeyboardState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<VirtualKeyboardState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        event: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        println!("virtual_keyboard_manager::event {:?}", event);
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for VirtualKeyboardState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        event: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        println!("virtual_keyboard::event {:?}", event);
    }
}
