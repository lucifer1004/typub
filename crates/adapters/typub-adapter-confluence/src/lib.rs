mod adapter;
mod config;
mod format;
mod model;

#[cfg(test)]
mod tests;

pub use config::{CAPABILITY, ID, create, register};
