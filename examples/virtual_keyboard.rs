use std::{io::{Seek, SeekFrom, Write}, os::fd::IntoRawFd, time::Duration};
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_protocols_async::zwp_virtual_keyboard_v1::{handler::{KeyMotion, VirtualKeyboardHandler, VirtualKeyboardMessage}, keymap::KEYMAP};
use tempfile::tempfile;

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the virtual_keyboard handler
    let (virtual_keyboard_msg_tx, mut virtual_keyboard_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the virtual_keyboard handler
    let (virtual_keyboard_event_tx, mut virtual_keyboard_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let (keymap_raw_fd, keymap_size) = load_keymap();
    let mut virtual_keyboard_handler: VirtualKeyboardHandler = VirtualKeyboardHandler::new(keymap_raw_fd, keymap_size, virtual_keyboard_event_tx);


    // start the virtual_keyboard handler
    let virtual_keyboard_t = tokio::spawn(async move {
        println!("running keyboard handler");
        let _ = virtual_keyboard_handler.run(virtual_keyboard_msg_rx).await;
    });

    // receive all virtual_keyboard events
    let virtual_keyboard_event_t = tokio::spawn(async move {
        loop {
            let msg = virtual_keyboard_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received virtual_keyboard_event event={:?}", msg);
        }
    });

    println!("init ready");


    // send an event
    let _ = tokio::spawn(async move {
        println!("Sending U button");
        let _ = sleep(Duration::from_secs(3)).await;
        println!("Sending U button 2");
        let _ = virtual_keyboard_msg_tx.send(VirtualKeyboardMessage::Key { keycode: 22, keymotion: KeyMotion::Press }).await;
        let _ = sleep(Duration::from_millis(100)).await;
        let _ = virtual_keyboard_msg_tx.send(VirtualKeyboardMessage::Key { keycode: 22, keymotion: KeyMotion::Release }).await;
    });

    let _ = virtual_keyboard_t.await.unwrap();
    let _ = virtual_keyboard_event_t.await.unwrap();
}

fn load_keymap() -> (i32, u32) {
     // Get the keymap the keyboard is supposed to get initialized with
     let src: &str = KEYMAP;
     let keymap_size = KEYMAP.len();
     let keymap_size_u32: u32 = keymap_size.try_into().unwrap(); // Convert it from usize to u32, panics if it is not possible
     let keymap_size_u64: u64 = keymap_size.try_into().unwrap(); // Convert it from usize to u64, panics if it is not possible
                                                                 // Create a temporary file
     let mut keymap_file = tempfile().expect("Unable to create tempfile");
     // Allocate the required space in the file first
     keymap_file.seek(SeekFrom::Start(keymap_size_u64)).unwrap();
     keymap_file.write_all(&[0]).unwrap();
     keymap_file.rewind().unwrap();
     // Memory map the file
     let mut data = unsafe {
         memmap2::MmapOptions::new()
             .map_mut(&keymap_file)
             .expect("Could not access data from memory mapped file")
     };
     // Write the keymap to it
     data[..src.len()].copy_from_slice(src.as_bytes());
     // Initialize the virtual keyboard with the keymap
     let keymap_raw_fd = keymap_file.into_raw_fd();

     (keymap_raw_fd, keymap_size_u32)
}
