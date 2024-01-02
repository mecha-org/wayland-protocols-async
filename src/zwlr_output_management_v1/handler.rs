use std::collections::HashMap;

use tokio::{sync::{mpsc::{Sender, Receiver}, oneshot}, io::unix::AsyncFd};
use wayland_client::{EventQueue, protocol::{wl_seat::{WlSeat, self}, wl_registry::{WlRegistry, self}, wl_output::{WlOutput, self, Transform}}, globals::{self, GlobalListContents}, Connection, QueueHandle, Dispatch, backend::{ObjectId, protocol::WEnumError}};
use wayland_protocols_wlr::output_management::v1::client::{zwlr_output_manager_v1::ZwlrOutputManagerV1, zwlr_output_configuration_v1::ZwlrOutputConfigurationV1, zwlr_output_head_v1::AdaptiveSyncState, zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1};
use super::errors::{OutputManagementHandlerError, OutputManagementHandlerErrorCodes};
use crate::zwlr_output_management_v1::output::{output_head::WlOutputHead, output_mode::WlOutputMode};

#[derive(Debug)]
pub enum OutputManagementMessage {
    GetHeads { reply_to: oneshot::Sender<Vec<OutputHeadMeta>> },
    GetHead { id: ObjectId, reply_to: oneshot::Sender<Result<OutputHeadMeta, OutputManagementHandlerError>> },
    GetMode { id: ObjectId, reply_to: oneshot::Sender<Result<OutputModeMeta, OutputManagementHandlerError>> },
    EnableHead { id: ObjectId, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    DisableHead { id: ObjectId, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetMode { head_id: ObjectId, mode_id: ObjectId, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetCustomMode { head_id: ObjectId, width: i32, height: i32, refresh: i32, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetPosition { head_id: ObjectId, x: i32, y: i32, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetTransform { head_id: ObjectId, transform: Transform, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetScale { head_id: ObjectId, scale: f64, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
    SetAdaptiveSync { head_id: ObjectId, enable: bool, reply_to: oneshot::Sender<Result<bool, OutputManagementHandlerError>> },
}

#[derive(Debug)]
pub enum OutputManagementEvent {
    Head { id: ObjectId },
    Done { serial: u32 },
    Finished,
    HeadName { id: ObjectId, name: String },
    HeadDescription { id: ObjectId, description: String },
    HeadPhysicalSize { id: ObjectId, height: i32, width: i32 },
    HeadMode { id: ObjectId, mode_id: ObjectId },
    HeadEnabled { id: ObjectId, enabled: bool },
    HeadCurrentMode { id: ObjectId, mode_id: ObjectId },
    HeadPosition { id: ObjectId, x: i32, y: i32 },
    HeadTransform { id: ObjectId, transform: Result<Transform, WEnumError> },
    HeadScale { id: ObjectId, scale: f64 },
    HeadFinished { id: ObjectId },
    HeadMake { id: ObjectId, make: String },
    HeadModel { id: ObjectId, model: String },
    HeadSerialNumber { id: ObjectId, serial_number: String },
    HeadAdaptiveSync { id: ObjectId, state: Result<AdaptiveSyncState, WEnumError> },
    ModeSize { height: i32, width: i32 },
    ModeRefresh { refresh: i32 },
    ModePreferred,
    ModeFinished,
    ConfigurationSucceeded,
    ConfigurationFailed,
    ConfigurationCancelled,
}

#[derive(Debug, Clone)]
pub struct OutputHeadMeta {
    pub id: ObjectId,
    pub adaptive_sync: Option<AdaptiveSyncState>,
    pub current_mode: Option<ObjectId>,
    pub description: String,
    pub enabled: bool,
    pub height: i32,
    pub make: String,
    pub model: String,
    pub modes: Vec<ObjectId>,
    pub name: String,
    pub pos_x: i32,
    pub pos_y: i32,
    pub scale: f64,
    pub transform: Option<Transform>,
    pub width: i32,
}

#[derive(Debug, Clone)]

pub struct OutputModeMeta {
    pub id: ObjectId,
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
    pub preferred: bool,
}

pub struct OutputManagementState {
    qh: QueueHandle<OutputManagementState>,
    event_tx: Sender<OutputManagementEvent>,
    output: WlOutput,
    pub output_manager: ZwlrOutputManagerV1,
    pub output_manager_serial: Option<u32>,
    pub output_heads: HashMap<ObjectId, WlOutputHead>,
    pub output_modes: HashMap<ObjectId, WlOutputMode>,
    pub output_configuration: Option<ZwlrOutputConfigurationV1>,
    pub mode_to_head_ids: HashMap<ObjectId, ObjectId>,
    seat: WlSeat,
}

pub struct OutputManagementHandler {
    event_queue: EventQueue<OutputManagementState>,
    state: OutputManagementState,
}

impl OutputManagementHandler {
    pub fn new(event_tx: Sender<OutputManagementEvent>) -> Self {
        let conn = wayland_client::Connection::connect_to_env()
            .map_err(|_| "could not connect to wayland socket, try setting WAYLAND_DISPLAY.")
            .unwrap();
        let (globals, mut event_queue) = globals::registry_queue_init::<OutputManagementState>(&conn).unwrap();
        let qh = event_queue.handle();

        let output_manager = globals
            .bind::<ZwlrOutputManagerV1, _, _>(&qh, core::ops::RangeInclusive::new(4, 4), ())
            .map_err(|_| "compositor does not implement output manager (v4).").unwrap();
        let seat = globals
            .bind::<WlSeat, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the seat from global.")
            .unwrap();
        let output = globals
            .bind::<WlOutput, _, _>(&qh, core::ops::RangeInclusive::new(1, 1), ())
            .map_err(|_| "failed to retrieve the output from global.")
            .unwrap();

        let mut state = OutputManagementState {
            event_tx,
            qh,
            seat,
            output,
            output_manager,
            output_manager_serial: None,
            output_heads: HashMap::new(),
            output_modes: HashMap::new(),
            mode_to_head_ids: HashMap::new(),
            output_configuration: None,
        };

        event_queue.roundtrip(&mut state).unwrap();
    
        OutputManagementHandler {
            event_queue,
            state,
        }
    }

    pub async fn run(&mut self, mut msg_rx: Receiver<OutputManagementMessage>) {
        let event_queue = &mut self.event_queue;
        let mut output_mgmt_state = &mut self.state;
    
        loop {
            // This would be required if other threads were reading from the socket.
            event_queue.dispatch_pending(&mut output_mgmt_state).unwrap();
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
                            event_queue.dispatch_pending(&mut output_mgmt_state).unwrap();
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
                        OutputManagementMessage::GetHeads { reply_to } => {
                            let heads = output_mgmt_state.get_heads();
                            let _ = reply_to.send(heads);
                        },
                        OutputManagementMessage::GetHead { id, reply_to } => {
                            let head = output_mgmt_state.get_head(id);
                            let _ = reply_to.send(head);
                        },
                        OutputManagementMessage::GetMode { id, reply_to } => {
                            let mode = output_mgmt_state.get_mode(id);
                            let _ = reply_to.send(mode);
                        },
                        OutputManagementMessage::EnableHead { id, reply_to } => {
                            let res = output_mgmt_state.enable_head(id);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::DisableHead { id, reply_to } => {
                            let res = output_mgmt_state.disable_head(id);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::SetMode { head_id, mode_id, reply_to } => {
                            let res = output_mgmt_state.set_mode(head_id, mode_id);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::SetCustomMode { head_id, width, height, refresh, reply_to } => {
                            let res = output_mgmt_state.set_custom_mode(head_id, width, height, refresh);
                            let _ = reply_to.send(res);
                        }
                        OutputManagementMessage::SetPosition { head_id, x, y, reply_to } => {
                            let res = output_mgmt_state.set_position(head_id, x, y);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::SetTransform { head_id, transform, reply_to } => {
                            let res = output_mgmt_state.set_transform(head_id, transform);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::SetScale { head_id, scale, reply_to } => {
                            let res = output_mgmt_state.set_scale(head_id, scale);
                            let _ = reply_to.send(res);
                        },
                        OutputManagementMessage::SetAdaptiveSync { head_id, enable, reply_to } => {
                            let res = output_mgmt_state.set_adaptive_sync(head_id, enable);
                            let _ = reply_to.send(res);
                        }
                    }
                }
            }
    
            // Send any new messages to the socket.
            let _ = event_queue.flush().expect("wayland connection closed");
        }
    }
}

impl OutputManagementState {
    pub fn dispatch_event(&self, event: OutputManagementEvent) {
        let tx: Sender<OutputManagementEvent> = self.event_tx.clone();
        tokio::task::spawn(async move {
            let _ = tx.send(event).await;
        });
    }

    fn generate_config(&self, head_id: &ObjectId) -> Result<(ZwlrOutputConfigurationV1, ZwlrOutputConfigurationHeadV1), OutputManagementHandlerError> {
        let output_manager = &self.output_manager;
        let serial = match self.output_manager_serial {
            Some(s) => s,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::NoSerialManagerNotReady,
                    format!("output serial not found, manager not ready for configuration")
                ))
            },
        };
        let head = match self.output_heads.get(head_id) {
            Some(h) => h,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::HeadNotFoundError,
                    format!("output head not found")
                ))
            }
        };

        let configuration = output_manager.create_configuration(serial, &self.qh, ());
        let head_config = configuration.enable_head(&head.output_head, &self.qh, ());
        
        Ok((configuration, head_config))
    }

    fn get_heads(&self) -> Vec<OutputHeadMeta> {
        let mut output_head_meta: Vec<OutputHeadMeta> = vec![];
        for (key, wl_output) in self.output_heads.iter() {
            output_head_meta.push(OutputHeadMeta {
                id: key.clone(),
                adaptive_sync: wl_output.adaptive_sync,
                current_mode: wl_output.current_mode.clone(),
                description: wl_output.description.clone(),
                enabled: wl_output.enabled,
                height: wl_output.height,
                make: wl_output.make.clone(),
                model: wl_output.model.clone(),
                modes: wl_output.modes.clone(),
                name: wl_output.name.clone(),
                pos_x: wl_output.pos_x,
                pos_y: wl_output.pos_y,
                scale: wl_output.scale,
                transform: wl_output.transform,
                width: wl_output.width,
            })
        }
        output_head_meta
    }

    fn get_head(&self, head_id: ObjectId) -> Result<OutputHeadMeta, OutputManagementHandlerError> {
        match &self.output_heads.get(&head_id) {
            Some(wl_output) => Ok(OutputHeadMeta {
                id: head_id.clone(),
                adaptive_sync: wl_output.adaptive_sync,
                current_mode: wl_output.current_mode.clone(),
                description: wl_output.description.clone(),
                enabled: wl_output.enabled,
                height: wl_output.height,
                make: wl_output.make.clone(),
                model: wl_output.model.clone(),
                modes: wl_output.modes.clone(),
                name: wl_output.name.clone(),
                pos_x: wl_output.pos_x,
                pos_y: wl_output.pos_y,
                scale: wl_output.scale,
                transform: wl_output.transform,
                width: wl_output.width,
            }),
            None => Err(OutputManagementHandlerError::new(
                OutputManagementHandlerErrorCodes::HeadNotFoundError,
                format!("output head not found")
            )),
        }
    }

    fn get_mode(&self, mode_id: ObjectId) -> Result<OutputModeMeta, OutputManagementHandlerError> {
        match &self.output_modes.get(&mode_id) {
            Some(wl_output_mode) => Ok(OutputModeMeta {
                id: mode_id.clone(),
                width: wl_output_mode.width,
                height: wl_output_mode.height,
                refresh: wl_output_mode.refresh,
                preferred: wl_output_mode.preferred,
            }),
            None => Err(OutputManagementHandlerError::new(
                OutputManagementHandlerErrorCodes::ModeNotFoundError,
                format!("output mode not found")
            )),
        }
    }

    pub fn enable_head(&self, head_id: ObjectId) -> Result<bool, OutputManagementHandlerError> {
        let output_manager = &self.output_manager;
        let serial = match self.output_manager_serial {
            Some(s) => s,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::NoSerialManagerNotReady,
                    format!("output serial not found, manager not ready for configuration")
                ))
            },
        };

        if self.output_heads.get(&head_id).is_none() {
            return Err(OutputManagementHandlerError::new(
                OutputManagementHandlerErrorCodes::HeadNotFoundError,
                format!("output head not found")
            ));
        }

        let head = match self.output_heads.get(&head_id) {
            Some(h) => h,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::HeadNotFoundError,
                    format!("output head not found")
                ))
            }
        };

        let configuration = output_manager.create_configuration(serial, &self.qh, ());
        let head_config = configuration.enable_head(&head.output_head, &self.qh, ());
        
        configuration.apply();

        Ok(true)
    }

    pub fn disable_head(&self, head_id: ObjectId) -> Result<bool, OutputManagementHandlerError> {
        let output_manager = &self.output_manager;
        let serial = match self.output_manager_serial {
            Some(s) => s,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::NoSerialManagerNotReady,
                    format!("output serial not found, manager not ready for configuration")
                ))
            },
        };

        let head = match self.output_heads.get(&head_id) {
            Some(h) => h,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::HeadNotFoundError,
                    format!("output head not found")
                ))
            }
        };

        let head = self.output_heads.get(&head_id).unwrap();
        let configuration = output_manager.create_configuration(serial, &self.qh, ());

        configuration.disable_head(&head.output_head);
        configuration.apply();

        Ok(true)
    }

    pub fn set_mode(&self, head_id: ObjectId, mode_id: ObjectId) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };

        let mode = match self.output_modes.get(&mode_id) {
            Some(h) => h,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::ModeNotFoundError,
                    format!("output mode not found")
                ))
            }
        };

        head_config.set_mode(&mode.wlr_mode);
        configuration.apply();

        Ok(true)
    }

    pub fn set_custom_mode(&self, head_id: ObjectId, width: i32, height: i32, refresh: i32) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };
        head_config.set_custom_mode(width, height, refresh);
        configuration.apply();
        Ok(true)
    }

    pub fn set_transform(&self, head_id: ObjectId, transform: Transform) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };
        head_config.set_transform(transform);
        configuration.apply();
        Ok(true)
    }

    pub fn set_position(&self, head_id: ObjectId, x: i32, y: i32) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };
        head_config.set_position(x, y);
        configuration.apply();
        Ok(true)
    }

    pub fn set_scale(&self, head_id: ObjectId, scale: f64) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };
        head_config.set_scale(scale);
        configuration.apply();
        Ok(true)
    }

    pub fn set_adaptive_sync(&self, head_id: ObjectId, enable: bool) -> Result<bool, OutputManagementHandlerError> {
        let (configuration, head_config) = match self.generate_config(&head_id) {
            Ok((c, h)) => (c, h),
            Err(e) => {
                return Err(e);
            },
        };
        let _ = match enable {
            true => head_config.set_adaptive_sync(AdaptiveSyncState::Enabled),
            false => head_config.set_adaptive_sync(AdaptiveSyncState::Disabled),
        };
        configuration.apply();
        Ok(true)
    }

    pub(crate) fn remove_mode(&mut self, id: &ObjectId) -> Result<(), OutputManagementHandlerError> {
        let head_id = match self
            .mode_to_head_ids
            .remove(&id) {
                Some(h) => h,
                None => {
                    return Err(OutputManagementHandlerError::new(
                        OutputManagementHandlerErrorCodes::RemoveModeError,
                        format!("remove mode failed, maybe mode not found")
                    ))
                },
            };

        let head = match self.output_heads.get_mut(&head_id) {
            Some(h) => h,
            None => {
                return Err(OutputManagementHandlerError::new(
                    OutputManagementHandlerErrorCodes::HeadNotFoundError,
                    format!("head not found")
                ));
            },
        };

        if let Some(mode_id) = &head.current_mode {
            if mode_id == id {
                head.current_mode = None;
            }
        }

        head.modes.retain(|e| e != id);

        Ok(())
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for OutputManagementState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<OutputManagementState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_seat" {
                registry.bind::<WlSeat, (), OutputManagementState>(name, version, qh, ());
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<OutputManagementState>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            state.seat = seat.to_owned();
        }
    }
}

impl Dispatch<WlOutput, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<OutputManagementState>,
    ) {
        if let wl_output::Event::Name { .. } = event {
            state.output = output.to_owned();
        }
    }
}
