
use wayland_client::{Dispatch, Connection, QueueHandle, Proxy, backend::ObjectId, protocol::wl_output::Transform};
use wayland_protocols_wlr::output_management::v1::client::{zwlr_output_head_v1::{ZwlrOutputHeadV1, self, AdaptiveSyncState}, zwlr_output_mode_v1::ZwlrOutputModeV1, zwlr_output_manager_v1::ZwlrOutputManagerV1};

use crate::zwlr_output_management_v1::{handler::OutputManagementEvent, output::output_mode::WlOutputMode};
use crate::zwlr_output_management_v1::handler::OutputManagementState;

#[derive(Debug, Clone)]
pub struct WlOutputHead {
    pub adaptive_sync: Option<AdaptiveSyncState>,
    pub current_mode: Option<ObjectId>,
    pub description: String,
    pub enabled: bool,
    pub height: i32,
    pub make: String,
    pub model: String,
    pub modes: Vec<ObjectId>,
    pub name: String,
    pub output_head: ZwlrOutputHeadV1,
    pub pos_x: i32,
    pub pos_y: i32,
    pub scale: f64,
    pub serial_number: String,
    pub transform: Option<Transform>,
    pub width: i32,
}

impl WlOutputHead {
    pub fn new(output_head: ZwlrOutputHeadV1) -> Self {
        Self {
            adaptive_sync: None,
            current_mode: None,
            description: String::new(),
            enabled: false,
            height: 0,
            make: String::new(),
            model: String::new(),
            modes: Vec::new(),
            name: String::new(),
            pos_x: 0,
            pos_y: 0,
            scale: 1.0,
            serial_number: String::new(),
            transform: None,
            output_head,
            width: 0,
        }
    }
}

impl Dispatch<ZwlrOutputHeadV1, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        head: &ZwlrOutputHeadV1,
        event: <ZwlrOutputHeadV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _handle: &QueueHandle<Self>,
    ) {
        let output_head = state
            .output_heads
            .entry(head.id())
            .or_insert_with(|| WlOutputHead::new(head.clone()));

        match event {
            zwlr_output_head_v1::Event::Name { name } => {
                output_head.name = name.clone();
                state.dispatch_event(OutputManagementEvent::HeadName { id: head.id(), name });
            },
            zwlr_output_head_v1::Event::Description { description } => {
                output_head.description = description.clone();
                state.dispatch_event(OutputManagementEvent::HeadDescription { id: head.id(), description });
            },
            zwlr_output_head_v1::Event::PhysicalSize { width, height } => {
                output_head.width = width;
                output_head.height = height;
                state.dispatch_event(OutputManagementEvent::HeadPhysicalSize { id: head.id(), height, width });
            },
            zwlr_output_head_v1::Event::Mode { mode } => {
                let mode_id = mode.id();
                state.mode_to_head_ids.insert(mode_id.clone(), head.id());
                output_head.modes.push(mode_id.clone());
                state.output_modes.insert(mode_id.clone(), WlOutputMode::new(mode));
                state.dispatch_event(OutputManagementEvent::HeadMode { id: head.id(), mode_id: mode_id });

            },
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                let enabled = match enabled {
                    0 => false,
                    _ => true
                };
                output_head.enabled = enabled;
                state.dispatch_event(OutputManagementEvent::HeadEnabled { id: head.id(), enabled })
            },
            zwlr_output_head_v1::Event::CurrentMode { mode } => {
                output_head.current_mode = Some(mode.id());
                state.dispatch_event(OutputManagementEvent::HeadCurrentMode { id: head.id(), mode_id: mode.id() });
            },
            zwlr_output_head_v1::Event::Position { x, y } => {
                output_head.pos_x = x;
                output_head.pos_y = y;
                state.dispatch_event(OutputManagementEvent::HeadPosition { id: head.id(), x, y });
            },
            zwlr_output_head_v1::Event::Transform { transform } => {
                let transform = transform.into_result();
                output_head.transform = transform.ok();
                state.dispatch_event(OutputManagementEvent::HeadTransform { id: head.id(), transform });
            },
            zwlr_output_head_v1::Event::Scale { scale } => {
                output_head.scale = scale;
                state.dispatch_event(OutputManagementEvent::HeadScale { id: head.id(), scale });
            },
            zwlr_output_head_v1::Event::Finished => {
                head.release();
                state.output_heads.remove(&head.id());
                state.dispatch_event(OutputManagementEvent::HeadFinished { id: head.id() });

            },
            zwlr_output_head_v1::Event::Make { make } => {
                output_head.make = make.clone();
                state.dispatch_event(OutputManagementEvent::HeadMake { id: head.id(), make });

            },
            zwlr_output_head_v1::Event::Model { model } => {
                output_head.model = model.clone();
                state.dispatch_event(OutputManagementEvent::HeadModel { id: head.id(), model });
            },
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                output_head.serial_number = serial_number.clone();
                state.dispatch_event(OutputManagementEvent::HeadSerialNumber { id: head.id(), serial_number });

            },
            zwlr_output_head_v1::Event::AdaptiveSync { state: adaptive_sync } => {
                let adaptive_sync = adaptive_sync.into_result();
                output_head.adaptive_sync = adaptive_sync.ok();
                state.dispatch_event(OutputManagementEvent::HeadAdaptiveSync { id: head.id(), state: adaptive_sync });
            },
            _ => {},
        }
    }

    wayland_client::event_created_child!(Self, ZwlrOutputManagerV1, [
        EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}
