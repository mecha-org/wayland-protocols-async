use std::time::Duration;
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_protocols_async::ext_session_lock_v1::handler::{SessionLockHandler, SessionLockMessage};

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the session_lock handler
    let (session_lock_msg_tx, mut session_lock_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the session_lock handler
    let (session_lock_event_tx, mut session_lock_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let mut session_lock_handler = SessionLockHandler::new(session_lock_event_tx);


    // start the session_lock handler
    let session_lock_t = tokio::spawn(async move {
        let _ = session_lock_handler.run(session_lock_msg_rx).await;
    });

    // receive all session_lock events
    let session_lock_event_t = tokio::spawn(async move {
        loop {
            let msg = session_lock_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received session_lock_event event={:?}", msg);
        }
    });

    // send an event
    let _ = tokio::spawn(async move {
        // trigger lock
        let _ = sleep(Duration::from_secs(5)).await;
        let (tx, rx) = oneshot::channel();
        let _ =session_lock_msg_tx.send(SessionLockMessage::Lock { wl_surface: None, reply_to: tx }).await;

        let res = rx.await.expect("no reply from session_lock handler");
        println!("locked session {:?}", res);

        // trigger unlock
        let _ = sleep(Duration::from_secs(5)).await;
        let (tx, rx) = oneshot::channel();
        let _ =session_lock_msg_tx.send(SessionLockMessage::Unlock { reply_to: tx }).await;

        let res = rx.await.expect("no reply from session_lock handler");
        println!("unlocked session {:?}", res);
    });

    let _ = session_lock_t.await.unwrap();
    let _ = session_lock_event_t.await.unwrap();
}
