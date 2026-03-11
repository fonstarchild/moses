// Library target for running tests without linking the full Tauri binary.
// The actual app entry point is main.rs.
#![allow(dead_code, unused_variables)]
pub mod agent;
pub mod llm;
pub mod memory;
pub mod patch;
pub mod security;
pub mod settings;
pub mod workspace;
