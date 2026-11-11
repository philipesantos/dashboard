// The app's host commands: the one list that `lib.rs` registers and `build.rs` hands to the ACL.
// Both files `include!` it, each with its own `commands!` macro.
//
// To add a command, follow the comment at the top of window-surface.toml. The window calls the
// widget host's three (specs/009-declarative-widget-ui/contracts/window-commands.md).
//
// Part of the security core (constitution, Principle IX).

commands![widgets_list, widget_render, widget_act];
