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

#[cfg(test)]
mod details_tests {
    use ::format::protocol15405::{TYPEINFOS, GAME_DETAILS_TYPEID};
    use ::versioned_serde::Deserializer;

    use serde::de;
    use serde_json::value::Value;
    
    const FILE: &'static [u8] = include_bytes!("../testdata/base_build_15405/replay.details");

    #[test]
    fn json_deserialize() {
        let mut de = Deserializer::new(FILE, TYPEINFOS, GAME_DETAILS_TYPEID);
        let stuff: Value = de::Deserialize::deserialize(&mut de).unwrap();
        panic!("stuff = {:?}", stuff);
    }
}


#[cfg(test)]
mod header_tests {

    // {
    //    "m_elapsedGameLoops": 25243,
    //    "m_signature": "StarCraft II replay\u001b11",
    //    "m_type": 2,
    //    "m_version": {
    //        "m_baseBuild": 15405,
    //        "m_build": 16223,
    //        "m_flags": 1,
    //        "m_major": 1,
    //        "m_minor": 0,
    //        "m_revision": 2
    //    }
    // }
    
    use ::format::protocol15405::{TYPEINFOS, REPLAY_HEADER_TYPEID};
    use ::versioned_serde::Deserializer;

    use serde::de;
    use serde_json::value::Value;
    
    const FILE_JSON: &'static str = include_str!("../testdata/header.json");
    const FILE: &'static [u8] = include_bytes!("../testdata/header");

    #[test]
    fn json_deserialize() {
        let expected: Value = ::serde_json::de::from_str(FILE_JSON).unwrap();
        let mut de = Deserializer::new(FILE, TYPEINFOS, REPLAY_HEADER_TYPEID);
        let result: Value = de::Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(expected, result);
    }
}