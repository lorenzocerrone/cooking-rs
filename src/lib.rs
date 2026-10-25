//! The cooking-rs library: every module of the app, shared by the wasm entry
//! point (`src/main.rs`) and the host-only authoring tool (`src/bin/scaffold.rs`).

pub mod app;
pub mod components;
pub mod extras;
pub mod filter;
pub mod format;
pub mod models;
pub mod pages;
pub mod pwa;
pub mod quantity;
pub mod recipes;
pub mod scaffold;
pub mod shopping;
pub mod state;
pub mod storage;
#[cfg(test)]
mod validate;
