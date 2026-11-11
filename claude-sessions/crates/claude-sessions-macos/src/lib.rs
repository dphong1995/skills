//! Every AppKit, CoreGraphics and Carbon call of the widget lives here: the only crate where `unsafe` is allowed.
//! No safe function takes a raw pointer; AppKit objects are held as `Retained<T>`.

pub mod accessibility;
pub mod alert;
pub mod app;
pub mod apple_script;
pub mod defaults;
#[cfg(feature = "hid")]
pub mod hid;
pub mod hotkeys;
pub mod keys;
pub mod login_item;
pub mod main_queue;
pub mod menu;
pub mod mouse;
mod observer;
pub mod panel;
pub mod screens;
pub mod sound;
pub mod text;
pub mod ui;
pub mod workspace;

pub use observer::NotificationObserver;
