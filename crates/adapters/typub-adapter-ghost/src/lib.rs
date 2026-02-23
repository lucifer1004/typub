mod adapter;
mod client;
mod config;
mod model;
mod types;

#[cfg(test)]
mod tests;

pub use adapter::GhostAdapter;
pub use config::{CAPABILITY, create, register};
pub use model::ID;
