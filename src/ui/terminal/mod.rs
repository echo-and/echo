// Adapted from gpui-terminal 0.1.0.
// Original project: https://github.com/zortax/gpui-terminal
// License: MIT OR Apache-2.0

#![allow(dead_code, clippy::module_inception)]

mod colors;
mod event;
mod input;
mod render;
mod terminal;
mod view;

pub use colors::ColorPalette;
pub use view::{TerminalConfig, TerminalView};
