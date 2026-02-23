mod adapter;
mod client;
mod config;
mod model;
mod types;

pub use adapter::WordPressAdapter;
pub use config::{CAPABILITY, create, register};
pub use model::ID;

#[cfg(test)]
mod tests;
