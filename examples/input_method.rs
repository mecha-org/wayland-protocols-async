use std::time::Duration;
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_protocols_async::zwp_input_method_v2::handler::InputMethodHandler;

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the input_method handler
    let (input_method_msg_tx, mut input_method_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the input_method handler
    let (input_method_event_tx, mut input_method_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let mut input_method_handler = InputMethodHandler::new(input_method_event_tx);


    // start the input_method handler
    let input_method_t = tokio::spawn(async move {
        let _ = input_method_handler.run(input_method_msg_rx).await;
    });

    // receive all input_method events
    let input_method_event_t = tokio::spawn(async move {
        loop {
            let msg = input_method_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received input_method_event event={:?}", msg);
        }
    });

    let _ = input_method_t.await.unwrap();
    let _ = input_method_event_t.await.unwrap();
}
