#![allow(dead_code)]
#![feature(plugin, custom_attribute, custom_derive)]
#![plugin(phf_macros)]
// #![plugin(serde_macros)]

extern crate phf;
extern crate serde;
extern crate serde_json;

#[cfg(test)]
mod tests;
pub mod common;
pub mod format;
mod versioned_serde;

pub use versioned_serde::Deserializer as VersionedDeserializer;

#[no_mangle]
fn quux() {}