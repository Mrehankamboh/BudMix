//! Miracast receiver for BudsLink.
//!
//! An Android phone casts its screen with the built-in "Cast" / "Screen cast"
//! feature (Miracast over Wi-Fi Direct). BudsLink is the receiver: it accepts
//! the connection through `Windows.Media.Miracast.MiracastReceiver` and plays
//! the stream with a `MediaPlayer`, so the phone's audio reaches the default
//! output (the buds), mixed with everything else Windows plays.
//!
//! * [`probe`]: pure logic that turns the receiver's status codes into a
//!   verdict and a plain-English explanation. Compiled and tested on every OS.
//! * `Receiver` (Windows only): starts the receiver and reports what happens.
//! * `MiracastBackend` (Windows only): the receiver, the Private-network
//!   check and the video window behind `budslink_core::CastBackend`.

pub mod probe;

#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod network;
#[cfg(windows)]
mod receiver;
#[cfg(windows)]
mod video;

#[cfg(windows)]
pub use backend::MiracastBackend;
#[cfg(windows)]
pub use network::connected_networks_private;
#[cfg(windows)]
pub use receiver::{Receiver, ReceiverEvent, ReceiverOptions, StartFailure};
#[cfg(windows)]
pub use video::VideoWindow;
