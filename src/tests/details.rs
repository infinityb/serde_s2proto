mod protocol15405 {
    use ::format::protocol15405::{TYPEINFOS, GAME_DETAILS_TYPEID};
    use ::versioned_serde::Deserializer;

    use serde::de;
    use serde_json::value::Value;
    
    const FILE: &'static [u8] = include_bytes!("../../testdata/base_build_15405/replay.details");

    #[test]
    fn json_deserialize() {
        let mut de = Deserializer::new(FILE, TYPEINFOS, GAME_DETAILS_TYPEID);
        let stuff: Value = de::Deserialize::deserialize(&mut de).unwrap();
        panic!("stuff = {:?}", stuff);
    }
}

mod protocol15405_II {
    use ::format::protocol15405::{TYPEINFOS, GAME_DETAILS_TYPEID};
    use ::versioned_serde::Deserializer;

    use serde::de;
    use serde_json::value::Value;
    
    const FILE: &'static [u8] = include_bytes!("../../testdata/base_build_15405/replay.details");

    #[test]
    fn json_deserialize() {
        let mut de = Deserializer::new(FILE, TYPEINFOS, GAME_DETAILS_TYPEID);
        let stuff: Value = de::Deserialize::deserialize(&mut de).unwrap();
        panic!("stuff = {:?}", stuff);
    }
}

