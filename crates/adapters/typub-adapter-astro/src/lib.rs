mod adapter;
mod config;

pub use adapter::AstroAdapter;
pub use config::ID;
pub use config::{CAPABILITY, create, register};

#[cfg(test)]
mod tests;
