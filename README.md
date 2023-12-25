# wayland-protocols-async
Wayland client protocols implemented in async as handlers using the Actor-model and tokio messaging

## Foreign Toplevel Management List
Implements the [`wlr-foreign-toplevel-management-unstable-v1 (ver: 3)`](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) protocol. All toplevels active and open are recorded in a `SlotMap` using a unique `ToplevelKey` that you can retrieve using messages or receive in events.

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

## Examples
Check examples under each protocol for more details
