use std::time::Duration;
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_protocols_async::zwlr_foreign_toplevel_management_v1::handler::{ToplevelHandler, ToplevelMessage};

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the toplevel handler
    let (toplevel_msg_tx, mut toplevel_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the toplevel handler
    let (toplevel_event_tx, mut toplevel_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let mut toplevel_handler = ToplevelHandler::new(toplevel_event_tx);


    // start the toplevel handler
    let toplevel_t = tokio::spawn(async move {
        let _ = toplevel_handler.run(toplevel_msg_rx).await;
    });

    // receive all toplevel events
    let toplevel_event_t = tokio::spawn(async move {
        loop {
            let msg = toplevel_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received toplevel_event event={:?}", msg);
        }
    });

    // send an event
    let toplevel_send_t = tokio::spawn(async move {
        let _ =sleep(Duration::from_secs(5)).await;
        let (tx, rx) = oneshot::channel();
        let _ =toplevel_msg_tx.send(ToplevelMessage::GetToplevels { reply_to: tx }).await;

        let res = rx.await.expect("no reply from toplevel handler");
        println!("received all toplevels ({:?})", res);
    });

    let _ = toplevel_t.await.unwrap();
    let _ = toplevel_event_t.await.unwrap();
    let _ = toplevel_send_t.await.unwrap();
}
