use std::{time::{Duration, self}, fs::File, os::fd::AsFd};
use image::{codecs::png::{PngEncoder, self}, ImageEncoder};
use tokio::{sync::{mpsc, oneshot}, time::sleep};
use wayland_protocols_async::zwlr_screencopy_v1::handler::{ScreencopyHandler, ScreencopyMessage, ScreencopyFrameOutput};

#[tokio::main]
async fn main() {
    // create mpsc channel for interacting with the screencopy handler
    let (screencopy_msg_tx, screencopy_msg_rx) = mpsc::channel(128);

    // create mpsc channel for receiving events from the screencopy handler
    let (screencopy_event_tx, mut screencopy_event_rx) = mpsc::channel(128);
    
    // create the handler instance
    let mut screencopy_handler = ScreencopyHandler::new(screencopy_event_tx);


    // start the screencopy handler
    let screencopy_t = tokio::spawn(async move {
        let _ = screencopy_handler.run(screencopy_msg_rx).await;
    });

    // receive all screencopy events
    let screencopy_event_t = tokio::spawn(async move {
        loop {
            let msg = screencopy_event_rx.recv().await;
            if msg.is_none() {
                continue;
            }
            println!("received screencopy_event event={:?}", msg);
        }
    });

    // send an event
    let _ = tokio::spawn(async move {
        // trigger capture output
        let _ = sleep(Duration::from_secs(2)).await;
        let (tx, rx) = oneshot::channel();
        let _ = screencopy_msg_tx.send(ScreencopyMessage::CaptureOutput { overlay_cursor: None, reply_to: tx }).await;

        // let reply = rx.blocking_recv();
        let _ = rx.await.expect("no reply from screencopy handler");

        // trigger copy frame
        let _ = sleep(Duration::from_secs(5)).await;

        let (tx, rx) = oneshot::channel();
        let _ = screencopy_msg_tx.send(ScreencopyMessage::CopyFrame { reply_to: tx }).await;

        let _ = sleep(Duration::from_secs(5)).await;

        let screencopy_frame_out = rx.await.expect("no reply from screencopy handler").unwrap();
        write_frame_to_file(screencopy_frame_out);
    });


    let _ = screencopy_t.await.unwrap();
    let _ = screencopy_event_t.await.unwrap();
}

fn write_frame_to_file(frame: ScreencopyFrameOutput) {
    let file_name = format!(
        "screenshot-{}.png",
        time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let mut writer = std::fs::File::create(&file_name).unwrap();
    let png_encoder = PngEncoder::new(&mut writer);
    let _ = png_encoder.write_image(&frame.file.unwrap(), frame.width, frame.height, image::ColorType::Rgba8);
        // .write_image(
        //     &frame.frame_mmap,
        //     frame.frameformat.width,
        //     frame.frameformat.height,
        //     image::ColorType::Rgba8,
        // )
}
