# wayland-protocols-async
Wayland client protocols implemented in async as handlers using the Actor-model and tokio messaging

## Foreign Toplevel Management List
Implements the [`zwlr-foreign-toplevel-management-unstable-v1 (ver: 3)`](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) protocol. All toplevels active and open are recorded in a `SlotMap` using a unique `ToplevelKey` that you can retrieve using messages or receive in events.

### Messages `ToplevelMessage`
List of messages supported are shown in the enum below
- **GetToplevels** - Returns all currently open toplevel keys
- **GetToplevels** - For a toplevel fetch its meta - title, app_id and state
- **Activate** - Set a toplevel to focus (using activate method)
- **SetFullscreen** - Set a toplevel to full screen
- **SetMaximize** - Set a toplevel to maximize (if your compositor supports)
- **SetMinimize** - Set a toplevel to minimize (if your compositor supports)
- **UnsetFullscreen** - Unset a toplevel from full screen
- **UnsetMaximize** - Unset a toplevel from maximize (if your compositor supports)
- **UnsetMinimize** - Unset a toplevel from minimize (if your compositor supports)
- **Close** - Close a toplevel application

### Events `ToplevelEvent`
List of events that can be received from the toplevel handler are
- **Created** - When a new toplevel is created
- **Title** - When a toplevel's `title` is updated
- **AppId** - When a toplevel `app_id` is updated
- **Done** - When all changes in the state are sent
- **State** - When the toplevel's state is updated, you can map the state based on this [enum](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1#zwlr_foreign_toplevel_handle_v1:enum:state0)
- **Closed** - When a toplevel is closed
- **OutputEnter** - When a toplevel becomes visible on an output
- **OutputLeave** - When a toplevel stops becoming visible on an output
- **Parent** - Whenever the parent of the toplevel changes.

## Input Method
Implements the [`zwp-input-method-unstable-v2 (ver: 1)`](https://github.com/Smithay/wayland-rs/blob/master/wayland-protocols-misc/protocols/input-method-unstable-v2.xml) protocol. Currently covers the `input_method` events and requests. This protocol is helpful for triggering virtual keyboards or sending key inputs.

### Messages `InputMethodMessage`
Currently no messages are implemented for this protocol.
- **CommitString** - Sends the commit string to the application
- **SetPreeditString** - Sends the pre-edit string text to the client application
- **DeleteSurroundingText** - Removes the surrounding text
- **Commit** - Apply state changes based on above events

### Events `InputMethodMessage`
List of events that can be received from the toplevel handler are
- **Activate** - When a text input is focused on the seat and requests the input method to be activated. You can use this to show the virtual keyboard.
- **Deactivate** - When there are no active or focused text input. You can use this to hide the virtual keyboard.
- **Done** - Triggered when all changes are applied.
- **SurroundingText** -  Returns the surrounding text with its cursor and selection anchor.
- **TextChangeCause** - Tells the input method why the text surrounding is changed
- **ContentType** - Indicates the content type and hint for the input method based on the interface.
- **Unavailable** - The input method is no longer available

**Note** - The `GetInputPopupSurface` and `GrabKeyboard` methods are not implemented and its related events.

## Examples
Check examples under each protocol for more details

## Credits
[`wayland.rs`](https://github.com/smithay/wayland-rs) - This project depends extensively on `smithay/wayland.rs` for its implementation of all wayland (stable, unstable, wlr, misc) protocols in rust.