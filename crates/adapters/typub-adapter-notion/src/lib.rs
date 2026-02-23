mod adapter;
mod blocks;
mod client;
mod config;
mod model;
mod spec;

pub use adapter::NotionAdapter;
pub use config::{CAPABILITY, create, register};
pub use model::ID;

#[cfg(test)]
mod tests;
