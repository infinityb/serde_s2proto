#![allow(dead_code)]
#![feature(slice_bytes, plugin, custom_attribute, custom_derive)]
#![plugin(phf_macros)]
// #![plugin(serde_macros)]

extern crate phf;
extern crate serde;
extern crate serde_json;

pub mod common;
pub mod format;
pub mod versioned_serde;

#[test]
fn it_works() {
}


#[no_mangle]
fn quux() {
	//
}