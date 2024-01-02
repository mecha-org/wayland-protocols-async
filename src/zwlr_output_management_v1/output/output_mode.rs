use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{ZwlrOutputModeV1, self};

use crate::zwlr_output_management_v1::handler::{OutputManagementEvent, OutputManagementState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WlOutputMode {
    pub width: i32,
    pub height: i32,
    pub refresh: i32,
    pub preferred: bool,
    pub wlr_mode: ZwlrOutputModeV1,
}

impl WlOutputMode {
    pub fn new(wlr_mode: ZwlrOutputModeV1) -> Self {
        Self {
            width: 0,
            height: 0,
            refresh: 0,
            preferred: false,
            wlr_mode,
        }
    }
}

impl Dispatch<ZwlrOutputModeV1, ()> for OutputManagementState {
    fn event(
        state: &mut Self,
        mode: &ZwlrOutputModeV1,
        event: <ZwlrOutputModeV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _handle: &QueueHandle<Self>,
    ) {
        let output_mode = state
            .output_modes
            .entry(mode.id())
            .or_insert_with(|| WlOutputMode::new(mode.clone()));

        println!("output_mode_v1::event event={:?}", event);

        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => {
                output_mode.width = width;
                output_mode.height = height;
                state.dispatch_event(OutputManagementEvent::ModeSize { height, width });
            },
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                output_mode.refresh = refresh;
                state.dispatch_event(OutputManagementEvent::ModeRefresh { refresh });

            },
            zwlr_output_mode_v1::Event::Preferred => {
                output_mode.preferred = true;
                state.dispatch_event(OutputManagementEvent::ModePreferred);
            },
            zwlr_output_mode_v1::Event::Finished => {
                mode.release();

                let _ = state.remove_mode(&mode.id());
                state.dispatch_event(OutputManagementEvent::ModeFinished);

                // if let Err(why) =  {
                //     tracing::error!(?why, id = ?proxy.id(), "failed to remove mode");
                // }
            },
            _ => {}
        }
    }
}
