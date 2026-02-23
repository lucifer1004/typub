mod adapter;
mod client;
mod config;
mod model;

#[cfg(test)]
mod tests;

pub use config::{CAPABILITY, ID, create, register};
