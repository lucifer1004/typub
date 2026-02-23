mod adapter;
mod config;

pub use adapter::StaticAdapter;
pub use config::ID;
pub use config::{CAPABILITY, create, register};

#[cfg(test)]
mod tests;
