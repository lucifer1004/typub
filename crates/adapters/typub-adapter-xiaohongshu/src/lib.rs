mod adapter;
mod config;

pub use adapter::XiaohongshuAdapter;
pub use config::ID;
pub use config::{CAPABILITY, create, register};

#[cfg(test)]
mod tests;
