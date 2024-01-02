use std::time::Duration;
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_client::protocol::wl_output::Transform;
use wayland_protocols_async::zwlr_output_management_v1::handler::{OutputManagementHandler, OutputManagementMessage};

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the output handler
    let (output_msg_tx, mut output_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the output handler
    let (output_event_tx, mut output_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let mut output_handler = OutputManagementHandler::new(output_event_tx);


    // start the output handler
    let output_t = tokio::spawn(async move {
        let _ = output_handler.run(output_msg_rx).await;
    });

    // receive all output events
    let output_event_t = tokio::spawn(async move {
        loop {
            let msg = output_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received output_event event={:?}", msg);
        }
    });

    let _ = output_t.await.unwrap();
    let _ = output_event_t.await.unwrap();
}
