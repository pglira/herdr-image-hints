//! One-key hints over the image paths in a Herdr pane; a hint shows the
//! image in a popup, drawn with the Kitty graphics protocol.
//!
//! `domain` is the pure kernel (parsing, matching, hint labels, the picking
//! state machine); `usecases` say what the plugin does against ports;
//! `adapters` implement those ports with Herdr, the terminal, the clipboard
//! and the file system; `app` wires them together for each CLI subcommand.

pub mod adapters;
pub mod app;
pub mod domain;
pub mod usecases;
