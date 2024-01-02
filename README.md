# wayland-protocols-async
Wayland client protocols implemented in async as handlers using the Actor-model and tokio messaging

## Foreign Toplevel Management List
Implements the [`zwlr-foreign-toplevel-management-unstable-v1 (ver: 3)`](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) protocol. All toplevels active and open are recorded in a `SlotMap` using a unique `ToplevelKey` that you can retrieve using messages or receive in events.

### Messages `ToplevelMessage`
List of messages supported are shown in the enum below
- **`GetToplevels`** - Returns all currently open toplevel keys
- **`GetToplevelMeta`** - For a toplevel fetch its meta - title, app_id and state
- **`Activate`** - Set a toplevel to focus (using activate method)
- **`SetFullscreen`** - Set a toplevel to full screen
- **`SetMaximize`** - Set a toplevel to maximize (if your compositor supports)
- **`SetMinimize`** - Set a toplevel to minimize (if your compositor supports)
- **`UnsetFullscreen`** - Unset a toplevel from full screen
- **`UnsetMaximize`** - Unset a toplevel from maximize (if your compositor supports)
- **`UnsetMinimize`** - Unset a toplevel from minimize (if your compositor supports)
- **`Close`** - Close a toplevel application

### Events `ToplevelEvent`
List of events that can be received from the toplevel handler are
- **`Created`** - When a new toplevel is created
- **`Title`** - When a toplevel's `title` is updated
- **`AppId`** - When a toplevel `app_id` is updated
- **`Done`** - When all changes in the state are sent
- **`State`** - When the toplevel's state is updated, the state is mapped base on this [enum](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1#zwlr_foreign_toplevel_handle_v1:enum:state0)
- **`Closed`** - When a toplevel is closed
- **`OutputEnter`** - When a toplevel becomes visible on an output
- **`OutputLeave`** - When a toplevel stops becoming visible on an output
- **`Parent`** - Whenever the parent of the toplevel changes.

## Input Method
Implements the [`zwp-input-method-unstable-v2 (ver: 1)`](https://github.com/Smithay/wayland-rs/blob/master/wayland-protocols-misc/protocols/input-method-unstable-v2.xml) protocol. Currently covers the `input_method` events and requests. This protocol is helpful for triggering virtual keyboards or sending key inputs.

### Messages `InputMethodMessage`
Currently below messages are implemented for this protocol.
- **`CommitString`** - Sends the commit string to the application
- **`SetPreeditString`** - Sends the pre-edit string text to the client application
- **`DeleteSurroundingText`** - Removes the surrounding text
- **`Commit`** - Apply state changes based on above events

### Events `InputMethodMessage`
List of events that can be received from the input method handler are
- **`Activate`** - When a text input is focused on the seat and requests the input method to be activated. You can use this to show the virtual keyboard.
- **`Deactivate`** - When there are no active or focused text input. You can use this to hide the virtual keyboard.
- **`Done`** - Triggered when all changes are applied.
- **`SurroundingText`** -  Returns the surrounding text with its cursor and selection anchor.
- **`TextChangeCause`** - Tells the input method why the text surrounding is changed
- **`ContentType`** - Indicates the content type and hint for the input method based on the interface.
- **`Unavailable`** - The input method is no longer available

**Note** - The `GetInputPopupSurface` and `GrabKeyboard` methods are not implemented and its related events.

## Output Management
Implements the [`zwlr-output-management-unstable-v1 (ver: 4)`](https://wayland.app/protocols/wlr-output-management-unstable-v1) protocol. This protocol enables you to receive events and metada related to the output, its heads and modes. You can use this protocol to configure the output with methods such as enabling or disabling heads, setting custom modes.


### Messages
- **`GetHeads`** - Returns all heads in the output along with their metadata, the `id` in the response can be used in head related operations
- **`GetHead`** - Returns a particular head based on the `id`
- **`GetMode`** - Returns a particular mode based on the `id` in the request
- **`EnableHead`** - Enables a head on the output
- **`DisableHead`** - Disables a head on the output
- **`SetMode`** - Sets a mode to the head
- **`SetCustomMode`** - Sets a custom mode (height, width, refresh rate) for a head
- **`SetPosition`** - Sets the position for the head
- **`SetTransform`** - Sets the transform (rotation) for the head
- **`SetScale`** - Sets the scale (zoom)
- **`SetAdaptiveSync`** - Enable or disable adaptive sync (if your compositor supports it)

### Events
- **`Head`** - When a new head appears or immediately when a new manager is bound
- **`Done`** - When all changes or events have been set for the output manager after it is bound. This is when the output manager is ready to receive configuration events
- **`Finished`** - When the compositor is finished with the manager and won't send any events or accept any configuration any more. The handler will need to be reinstantiated at this point or killed.
- **`HeadName`** - Describes the head's name
- **`HeadDescription`** - Human-readable description of the head
- **`HeadPhysicalSize`** - Physical size of the head
- **`HeadMode`** - Introduces a new supported mode for the head
- **`HeadEnabled`** - Emitted when a head is enabled
- **`HeadCurrentMode`** - Describes the current mode used by head, only sent when output is enabled
- **`HeadPosition`** - Describes position of the head in the compositor space
- **`HeadTransform`** - Current transformation applied to the head
- **`HeadScale`** - Scale value of the head
- **`HeadFinished`** - Indicates that the head is no longer available. Clients should destroy any references to it.
- **`HeadMake`** - Describe the manufacturer of the head
- **`HeadModel`** - Describes the model of the head
- **`HeadSerialNumber`** - Describes the serial number of the head
- **`HeadAdaptiveSync`** - Returns status (enabled or disabled) for AdaptiveSync
- **`ModeSize`** - Describes mode size in the physical hardware units of the output device
- **`ModeRefresh`** - Describes the mode's vertical refresh rate
- **`ModePreferred`** - Emitted when a mode is preferred by the output head
- **`ModeFinished`** - Indicates that the mode is no longer available. Clients should destroy any references to it.
- **`ConfigurationSucceeded`** - Sent after a compositor has successfuly applied all changes sent in configuration request
- **`ConfigurationFailed`** - Sent if a configuration has failed or the compositor has rejected them 
- **`ConfigurationCancelled`** - Sent if the compositor cancels a configuration request due to any change in the output


## Examples
Check the examples for sample on how to use the protocols

## Credits
- [`wayland.rs`](https://github.com/smithay/wayland-rs) - This project depends extensively on `smithay/wayland.rs` for its implementation of all wayland (stable, unstable, wlr, misc) protocols in rust.
- [`cosmic-randr`](https://github.com/pop-os/cosmic-randr) - For showing how to get managing events and state right for output management