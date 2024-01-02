use wayland_client::{Dispatch, Connection, QueueHandle, Proxy};
use wayland_protocols_wlr::output_management::v1::client::{zwlr_output_manager_v1::{ZwlrOutputManagerV1, self}, zwlr_output_head_v1::ZwlrOutputHeadV1};

use crate::zwlr_output_management_v1::{handler::{OutputManagementEvent, OutputManagementState}, output::output_head::WlOutputHead};

impl Dispatch<ZwlrOutputManagerV1, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        _: &ZwlrOutputManagerV1,
        event: <ZwlrOutputManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => {
                let head_id = head.clone().id();
                state.output_heads.insert(head.id(), WlOutputHead::new(head));
                state.dispatch_event(OutputManagementEvent::Head { id: head_id });
            },
            zwlr_output_manager_v1::Event::Done { serial } => {
                state.output_manager_serial = Some(serial);
                state.dispatch_event(OutputManagementEvent::Done { serial });
            },
            zwlr_output_manager_v1::Event::Finished => {
                state.output_manager_serial = None;
                state.dispatch_event(OutputManagementEvent::Finished);
            },
            _ => {},
        }
    }

    wayland_client::event_created_child!(Self, ZwlrOutputManagerV1, [
        EVT_HEAD_OPCODE=> (ZwlrOutputHeadV1, ()),
    ]);
}
