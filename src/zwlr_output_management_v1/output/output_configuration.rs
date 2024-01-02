use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_v1::{ZwlrOutputConfigurationV1, self};

use crate::zwlr_output_management_v1::handler::OutputManagementEvent;

use crate::zwlr_output_management_v1::handler::OutputManagementState;

impl Dispatch<ZwlrOutputConfigurationV1, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        mode: &ZwlrOutputConfigurationV1,
        event: <ZwlrOutputConfigurationV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _handle: &QueueHandle<Self>,
    ) {
        println!("output_configuration_v1::event event={:?}", event);

        match event {
            zwlr_output_configuration_v1::Event::Succeeded => {
                state.dispatch_event(OutputManagementEvent::ConfigurationSucceeded);
            }
            zwlr_output_configuration_v1::Event::Failed => {
                state.dispatch_event(OutputManagementEvent::ConfigurationFailed);
            }
            zwlr_output_configuration_v1::Event::Cancelled => {
                state.dispatch_event(OutputManagementEvent::ConfigurationCancelled);
            }
            _ => {},
        }
    }
}
