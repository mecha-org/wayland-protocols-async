use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc;
use wayland_protocols_async::ext_idle_notify_v1::handler::IdleNotifyHandler;

#[tokio::main]
async fn main() {
    // create mpsc channel for receiving events from the idle_notify handler
    let (idle_notify_event_tx, mut idle_notify_event_rx) = mpsc::channel(128);
    

    let mut subscribers = HashMap::new();

    subscribers.insert(String::from("1"), Duration::from_secs(5));
    subscribers.insert(String::from("2"), Duration::from_secs(10));


    // create the handler instance
    let mut idle_notify_handler = IdleNotifyHandler::new(subscribers, idle_notify_event_tx);


    // start the idle_notify handler
    let idle_notify_t = tokio::spawn(async move {
        let _ = idle_notify_handler.run().await;
    });

    // receive all idle_notify events
    let idle_notify_event_t = tokio::spawn(async move {
        loop {
            let msg = idle_notify_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received idle_notify_event event={:?}", msg);
        }
    });

    let _ = idle_notify_t.await.unwrap();
    let _ = idle_notify_event_t.await.unwrap();
}
