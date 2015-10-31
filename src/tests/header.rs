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

const FILE_JSON: &'static str = include_str!("../../testdata/header.json");
const FILE: &'static [u8] = include_bytes!("../../testdata/header");

#[test]
fn json_deserialize() {
    let expected: Value = ::serde_json::de::from_str(FILE_JSON).unwrap();
    let mut de = Deserializer::new(FILE, TYPEINFOS, REPLAY_HEADER_TYPEID);
    let result: Value = de::Deserialize::deserialize(&mut de).unwrap();
    assert_eq!(expected, result);
}
