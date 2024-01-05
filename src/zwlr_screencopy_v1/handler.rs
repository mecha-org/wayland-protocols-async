use std::{collections::HashSet, time::UNIX_EPOCH, time::SystemTime, ffi::{CStr, CString}, fs::File, os::fd::{AsFd, AsRawFd}, io::Error};
use memmap2::MmapMut;
use shmemfdrs::create_shmem;
use tokio::{sync::{mpsc::{Sender, Receiver}, oneshot}, io::unix::AsyncFd};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self}, wl_buffer::WlBuffer, wl_shm_pool::WlShmPool, wl_shm::{self, WlShm}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch, backend::protocol::WEnumError};
use wayland_protocols_wlr::screencopy::v1::client::{zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, zwlr_screencopy_frame_v1::{ZwlrScreencopyFrameV1, self}};

use super::errors::{ScreencopyHandlerError, ScreencopyHandlerErrorCodes};

#[derive(Debug)]
pub enum ScreencopyMessage {
    CaptureOutput { overlay_cursor: Option<i32>, reply_to: oneshot::Sender<Result<bool, ScreencopyHandlerError>> },
    CaptureOutputRegion { x: i32, y: i32, width: i32, height: i32, overlay_cursor: Option<i32>, reply_to: oneshot::Sender<Result<bool, ScreencopyHandlerError>> },
    CopyFrame { reply_to: oneshot::Sender<Result<ScreencopyFrameOutput, ScreencopyHandlerError>> },
    CopyWithDamage { reply_to: oneshot::Sender<bool> }
}

#[derive(Debug)]
pub enum ScreencopyEvent {
    Buffer { format: Result<wl_shm::Format, WEnumError>, width: u32, height: u32, stride: u32 },
    Flags { y_invert: Result<zwlr_screencopy_frame_v1::Flags, WEnumError> },
    Ready { tv_sec_hi: u32, tv_sec_lo: u32, tv_nsec: u32 },
    Failed,
    Damage { x: u32, y: u32, width: u32, height: u32 },
    LinuxDmabuf { format: u32, width: u32, height: u32 },
    BufferDone,
}

#[derive(Debug)]
pub struct ScreencopyFrameBuffer {
    pub format: Result<wl_shm::Format, WEnumError>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct ScreencopyFrameOutput {
    pub file: Result<MmapMut, Error>,
    pub format: wl_shm::Format,
    pub width: u32,
    pub height: u32,
}

pub struct ScreencopyState {
    qh: QueueHandle<ScreencopyState>,
    event_tx: Sender<ScreencopyEvent>,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,
    shm: Option<WlShm>,
    screencopy_manager: ZwlrScreencopyManagerV1,
    screencopy_frame: Option<ZwlrScreencopyFrameV1>,
    screencopy_frame_buffer: Option<ScreencopyFrameBuffer>,
    supported_buffer_formats: HashSet<wl_shm::Format>,
}

pub struct ScreencopyHandler {
    event_queue: EventQueue<ScreencopyState>,
    state: ScreencopyState,
}

impl ScreencopyHandler {
    pub fn new(event_tx: Sender<ScreencopyEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<ScreencopyState>(&conn).unwrap();
        let qh = event_queue.handle();
        let screencopy_manager = globals
            .bind::<ZwlrScreencopyManagerV1, _, _>(&qh, core::ops::RangeInclusive::new(3, 3), ())
            .map_err(|_| "compositor does not implement screencopy manager (v3).").unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();
        let shm: WlShm = globals
        .bind::<WlShm, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
        .map_err(|_| "failed to retrieve the shm from global.")
        .unwrap();

        let mut state = ScreencopyState {
            qh,
            event_tx,
            seat: Some(seat),
            output: Some(output),
            shm: Some(shm),
            screencopy_manager,
            screencopy_frame: None,
            screencopy_frame_buffer: None,
            supported_buffer_formats: HashSet::new(),
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        ScreencopyHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<ScreencopyMessage>) {
        let event_queue = &mut self.event_queue;
        let mut screencopy_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut screencopy_state).unwrap();
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
                            event_queue.dispatch_pending(&mut screencopy_state).unwrap();
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
                        ScreencopyMessage::CaptureOutput { overlay_cursor, reply_to } => {
                            let res = screencopy_state.capture_output(overlay_cursor);
                            let _ =reply_to.send(res);
                        },
                        ScreencopyMessage::CaptureOutputRegion { x, y, width, height, overlay_cursor, reply_to } => {
                            let res = screencopy_state.capture_output_region(x, y, width, height, overlay_cursor);
                            let _ =reply_to.send(res);
                        },
                        
                        ScreencopyMessage::CopyFrame { reply_to } => {
                            let res = screencopy_state.copy();
                            let _ =reply_to.send(res);
                        },
                        _ => {}
                    }
                }
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl ScreencopyState {
    fn dispatch_event(&self, event: ScreencopyEvent) {
        let tx: Sender<ScreencopyEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }

    fn capture_output(&mut self, overlay_cursor: Option<i32>) -> Result<bool, ScreencopyHandlerError> {
        let qh = &self.qh;
        let output = match &self.output {
            Some(o) => o,
            None => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::WLOutputIsNotSet,
                    format!("wl_output uninitialized or is not set")
                ));
            },
        };
        self.screencopy_frame = Some(self.screencopy_manager.capture_output(overlay_cursor.unwrap_or(0), output, qh, ()));
        Ok(true)
    }
    
    fn capture_output_region(&mut self, x: i32, y: i32, width: i32, height: i32, overlay_cursor: Option<i32>) -> Result<bool, ScreencopyHandlerError> {
        let qh = &self.qh;
        let output = match &self.output {
            Some(o) => o,
            None => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::WLOutputIsNotSet,
                    format!("wl_output uninitialized or is not set")
                ));
            },
        };
        self.screencopy_frame = Some(self.screencopy_manager.capture_output_region(
            overlay_cursor.unwrap_or(0),
            output,
            x,
            y,
            width,
            height,
            qh, ()));
        Ok(true)
    }

    fn copy(&mut self) -> Result<ScreencopyFrameOutput, ScreencopyHandlerError> {
        let qh = &self.qh;
        let shm = match &self.shm {
            Some(f) => f,
            None => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::WlShmIsNotSet,
                    format!("wl_shm uninitialized or is not set")
                ));
            },
        };
        let screencopy_frame = match &self.screencopy_frame {
            Some(f) => f,
            None => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::ScreencopyFrameIsNotSet,
                    format!("screencopy_frame uninitialized or is not set, did you capture output?")
                ));
            },
        };
        let frame_buffer_info = match &self.screencopy_frame_buffer {
            Some(f) => f,
            None => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::ScreencopyBufferIsNotSet,
                    format!("screencopy_frame_buffer uninitialized or is not set, did you wait for BufferDone?")
                ));
            },
        };

        let frame_buffer_format = match frame_buffer_info.format {
            Ok(f) => f,
            Err(_) => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::ScreencopyFrameBufferFormatError,
                    format!("screencopy_frame_buffer format is unavailable, invalid format from wayland server")
                ));
            },
        };

        if !self.supported_buffer_formats.contains(&frame_buffer_format) {
            return Err(ScreencopyHandlerError::new(
                ScreencopyHandlerErrorCodes::ScreencopyFrameBufferFormatUnsupported,
                format!("screencopy_frame_buffer format is not supported by the wl_shm formats")
            ));
        }
    
        let frame_buffer_size_bytes = frame_buffer_info.stride * frame_buffer_info.height;

        // create shared memory file descriptor
        let buffer_name = CString::new(format!("/screencopy_buffer_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos())).unwrap();
        let frame_buffer_fd = match create_shmem(buffer_name.as_c_str(), frame_buffer_size_bytes.try_into().unwrap()) {
            Ok(fd) => fd,
            Err(_) => {
                return Err(ScreencopyHandlerError::new(
                    ScreencopyHandlerErrorCodes::SharedMemFdCreateFailed,
                    format!("failed creating a shared mem fd")
                ));
            },
        };
        let frame_buffer_file = File::from(frame_buffer_fd);
        // frame_buffer_file.set_len(frame_buffer_size_bytes as u64);

        let shm_pool = shm.create_pool(
            frame_buffer_file.as_fd().as_raw_fd(),
            frame_buffer_size_bytes as i32,
            qh,
            (),
        );

        let buffer = shm_pool.create_buffer(
            0,
            frame_buffer_info.width as i32,
            frame_buffer_info.height as i32,
            frame_buffer_info.stride as i32,
            frame_buffer_format,
            qh,
            (),
        );

        // send the buffer for copy
        screencopy_frame.copy(&buffer);

        Ok(ScreencopyFrameOutput {
            file: unsafe { MmapMut::map_mut(&frame_buffer_file) },
            format: frame_buffer_format,
            width: frame_buffer_info.width,
            height: frame_buffer_info.height,
        })
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for ScreencopyState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<ScreencopyState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), ScreencopyState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<ScreencopyState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = Some(seat.to_owned());
        }
    }
}

impl Dispatch<WlOutput, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<ScreencopyState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = Some(output.to_owned());
        }
    }
}

impl Dispatch<WlShm, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        _proxy: &WlShm,
        event: <WlShm as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_shm::Event::Format { format } => {
                let format = format.into_result();
                if format.is_ok() {
                    state.supported_buffer_formats.insert(format.unwrap());
                }
            },
            _ => {},
        };
    }
}

impl Dispatch<WlShmPool, ()> for ScreencopyState {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlBuffer, ()> for ScreencopyState {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_buffer::Event::Release => {},
            _ => {},
        };
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for ScreencopyState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        _: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                state.screencopy_frame_buffer = Some(ScreencopyFrameBuffer {
                    format: format.into_result(),
                    width,
                    height,
                    stride,
                });
                state.dispatch_event(ScreencopyEvent::Buffer { format: format.into_result(), width, height, stride })
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::Flags { flags } => {
                state.dispatch_event(ScreencopyEvent::Flags { y_invert: flags.into_result() })
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                state.dispatch_event(ScreencopyEvent::Ready { tv_sec_hi, tv_sec_lo, tv_nsec })
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::Failed => {
                state.dispatch_event(ScreencopyEvent::Failed);
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::Damage { x, y, width, height } => {
                state.dispatch_event(ScreencopyEvent::Damage { x, y, width, height });
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::LinuxDmabuf { format, width, height } => {
                state.dispatch_event(ScreencopyEvent::LinuxDmabuf { format, width, height });
            },
            wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event::BufferDone => {
                state.dispatch_event(ScreencopyEvent::BufferDone);
            },
            _ => todo!(),
        };
    }
}
