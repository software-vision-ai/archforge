//! The project database's server: runs `dolt sql-server` for one project folder beside the
//! application, reachable only by this user with a password made for each start, connects to it,
//! stops it, and recovers a server a crashed Archforge left behind.
//!
//! Host services: the application uses this crate and the interface never depends on it. Dolt
//! embeds only into Go programs, so it runs as a separate process; everything above this crate
//! talks SQL over the connection [`Server::connect`] gives.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod binary;
mod config;
mod error;
mod home;
mod identity;
mod paths;
mod record;
mod recovery;
mod run_dir;
mod runner;
mod secret;
mod server;

pub use binary::Dolt;
pub use error::Error;
pub use home::Home;
pub use secret::Secret;
pub use server::{Server, ServerOptions, recover_only as recover};

/// This user's id, read from a folder made for the purpose in the temporary folder and removed
/// again: for checking who owns a project folder without unsafe code.
#[cfg(unix)]
pub fn user_id() -> std::io::Result<u32> {
    home::current_uid(&std::env::temp_dir())
}
