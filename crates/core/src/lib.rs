//! RustCraft core: everything a Minecraft: Java Edition launcher needs except
//! the UI, the JVM and the graphics translation layer.
//!
//! Design goals:
//! * no async runtime, no hidden background threads at rest;
//! * streaming downloads (never hold a whole jar/asset in RAM);
//! * all platform-specific paths are injected, so the same code runs in the
//!   Android app and in the desktop CLI.

pub mod auth;
pub mod download;
pub mod error;
pub mod install;
pub mod launch;
pub mod manifest;
pub mod paths;
pub mod profile;
pub mod rules;
pub mod session;

pub use error::{Error, Result};

pub const LAUNCHER_NAME: &str = "RustCraft";
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const USER_AGENT: &str = concat!("RustCraft/", env!("CARGO_PKG_VERSION"));
