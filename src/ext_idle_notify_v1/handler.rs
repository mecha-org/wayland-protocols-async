use std::{collections::HashMap, time::Duration};
use tokio::{io::unix::AsyncFd, sync::{mpsc::{Receiver, Sender}, oneshot}};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch};
use wayland_protocols::ext::idle_notify::v1::client::{ext_idle_notification_v1::{self, ExtIdleNotificationV1}, ext_idle_notifier_v1::ExtIdleNotifierV1};

// #[derive(Debug)]
// pub enum IdleNotifyMessage {}

#[derive(Debug)]
pub enum IdleNotifyEvent {
    Idled { key: String },
    Resumed { key: String },
}

pub struct IdleNotifyState {
    event_tx: Sender<IdleNotifyEvent>,
    _idle_notifier: ExtIdleNotifierV1,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
}

pub struct IdleNotifyHandler {
    event_queue: EventQueue<IdleNotifyState>,
    state: IdleNotifyState,
}

impl IdleNotifyHandler {
    pub fn new(subscribers: HashMap<String, Duration>, event_tx: Sender<IdleNotifyEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<IdleNotifyState>(&conn).unwrap();
        let qh = event_queue.handle();

        let idle_notifier = globals
            .bind::<ExtIdleNotifierV1, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "compositor does not implement input method (v1).").unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();

        for (key, timeout) in subscribers {
            let _ = idle_notifier.get_idle_notification(timeout.as_millis() as u32, &seat, &qh, key);
        }

        let mut state = IdleNotifyState {
            event_tx,
            _idle_notifier: idle_notifier,
            seat: Some(seat),
            output: Some(output),
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        IdleNotifyHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self) {
        let event_queue = &mut self.event_queue;
        let mut idle_notify_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut idle_notify_state).unwrap();
            let read_guard = event_queue.prepare_read().unwrap();
            let fd = read_guard.connection_fd();
            let async_fd = AsyncFd::new(fd).unwrap();
    
            tokio::select! {
                async_guard = async_fd.readable() => {
                    async_guard.unwrap().clear_ready();
                    // Drop the async_fd since it's holding a reference to the read_guard,
                    // which is dropped on read. We don't need to read from it anyways.
                    std::mem::drop(async_fd);
                    // This should` not block because we already ensured readiness
                    let event = read_guard.read();
                    match event {
                        // There are events but another thread processed them, we don't need to dispatch
                        Ok(0) => {},
                        // We have some events
                        Ok(_) => {
                            event_queue.dispatch_pending(&mut idle_notify_state).unwrap();
                        },
                        // No events to receive
                        Err(_) => {}
                        // Err(e) => eprintln!("error reading event {}", e),
                    }
                },
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl IdleNotifyState {
    fn dispatch_event(&self, event: IdleNotifyEvent) {
        let tx: Sender<IdleNotifyEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for IdleNotifyState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<IdleNotifyState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), IdleNotifyState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for IdleNotifyState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<IdleNotifyState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
            println!("new seat added");
        }
    }
}

impl Dispatch<WlOutput, ()> for IdleNotifyState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<IdleNotifyState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for IdleNotifyState {
    fn event(
        _: &mut Self,
        _: &ExtIdleNotifierV1,
        _: <ExtIdleNotifierV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotificationV1, String> for IdleNotifyState {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: <ExtIdleNotificationV1 as wayland_client::Proxy>::Event,
        key: &String,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                state.dispatch_event(IdleNotifyEvent::Idled { key: key.clone() });
            }
            ext_idle_notification_v1::Event::Resumed => {
                state.dispatch_event(IdleNotifyEvent::Resumed { key: key.clone() });
            },
            _ => {},
        };
    }
}