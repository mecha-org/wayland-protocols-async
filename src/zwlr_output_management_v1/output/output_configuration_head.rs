use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1;

use crate::zwlr_output_management_v1::handler::OutputManagementState;

impl Dispatch<ZwlrOutputConfigurationHeadV1, ()> for OutputManagementState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrOutputConfigurationHeadV1,
        event: <ZwlrOutputConfigurationHeadV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _handle: &QueueHandle<Self>,
    ) {}
}
