use serde;
use serde::de::Error;


#[derive(Serialize, Deserialize, Debug)]
pub struct Color {
    #[serde(rename="m_a")]
    pub a: u8,
    #[serde(rename="m_r")]
    pub r: u8,
    #[serde(rename="m_g")]
    pub g: u8,
    #[serde(rename="m_b")]
    pub b: u8,
}

impl serde::Deserialize for Color {
    fn deserialize<D>(deserializer: &mut D) -> Result<Color, D::Error>
        where D: serde::de::Deserializer
    {
        println!("setting up ColorField Deserialization");

        static FIELDS: &'static [&'static str] = &["m_a", "m_r", "m_g", "m_b"];
        deserializer.visit_struct("Color", FIELDS, ColorVisitor)
    }
}

pub struct ColorVersionVisitor;

impl serde::de::Visitor for ColorVersionVisitor {
    type Value = Color;
}

#[derive(Debug)]
enum ColorField {
    A,
    R,
    G, 
    B,
}

impl serde::Deserialize for ColorField {
    fn deserialize<D>(deserializer: &mut D) -> Result<ColorField, D::Error>
        where D: serde::de::Deserializer
    {
        struct ColorFieldVisitor;

        impl serde::de::Visitor for ColorFieldVisitor {
            type Value = ColorField;

            fn visit_bool<E>(&mut self, v: bool) -> Result<Self::Value, E> where E: serde::de::Error {
                panic!("visit_bool: {:?}", v);
            }

            fn visit_isize<E>(&mut self, v: isize) -> Result<Self::Value, E> where E: serde::de::Error {
                panic!("visit_isize: {:?}", v);
            }

            fn visit_i32<E>(&mut self, v: i32) -> Result<Self::Value, E> where E: serde::de::Error {
                panic!("visit_i32: {:?}", v);
            }

            fn visit_string<E>(&mut self, v: String) -> Result<Self::Value, E> where E: serde::de::Error {
                panic!("visit_string: {:?}", v);
            }

            fn visit_str<E>(&mut self, value: &str) -> Result<ColorField, E>
                where E: serde::de::Error
            {
                println!("ColorFieldVisitor::visit_str({:?})", value);
                match value {
                    "m_a" => Ok(ColorField::A),
                    "m_r" => Ok(ColorField::R),
                    "m_g" => Ok(ColorField::G),
                    "m_b" => Ok(ColorField::B),
                    _ => Err(serde::de::Error::syntax("expected value in {m_a, m_r, m_g, m_b}")),
                }
            }
        }

        println!("setting up ColorField Deserialization");
        deserializer.visit(ColorFieldVisitor)
    }
}

pub struct ColorVisitor;

impl serde::de::Visitor for ColorVisitor {
    type Value = Color;

    fn visit_map<V>(&mut self, mut visitor: V) -> Result<Self::Value, V::Error>
        where V: serde::de::MapVisitor
    {
        let mut a = None;
        let mut r = None;
        let mut g = None;
        let mut b = None;

        loop {
            
            println!("iterating on map visitor");
            let key_vis = visitor.visit_key();
            println!("key_vis.ok() => {:?}", key_vis.as_ref().ok());
            if key_vis.is_err() {
                panic!("key_vis is error");
            }

            match try!(key_vis) {
                Some(ColorField::A) => {
                    let val = try!(visitor.visit_value());
                    println!("setting a channel = {}", val);
                    a = Some(val);
                },
                Some(ColorField::R) => {
                    let val = try!(visitor.visit_value());
                    println!("setting r channel = {}", val);
                    r = Some(val);
                },
                Some(ColorField::G) => {
                    let val = try!(visitor.visit_value());
                    println!("setting g channel = {}", val);
                    g = Some(val);
                },
                Some(ColorField::B) => {
                    let val = try!(visitor.visit_value());
                    println!("setting b channel = {}", val);
                    b = Some(val);
                },
                None => { break; }
            }
        }

        let a = try!(a.ok_or(V::Error::missing_field("m_a")));
        let r = try!(r.ok_or(V::Error::missing_field("m_r")));
        let g = try!(g.ok_or(V::Error::missing_field("m_g")));
        let b = try!(b.ok_or(V::Error::missing_field("m_b")));
        try!(visitor.end());

        Ok(Color { a: a, r: r, g: g, b: b })
    }
}

#[cfg(test)]
mod tests {
    use ::format::protocol15405::{TYPEINFOS, REPLAY_HEADER_TYPEID};
    use ::versioned_serde::{Deserializer};
    use super::Color;
    use serde::de;


    const HEADER: &'static [u8] = &[
        0x05, 0x08, 0x00, 0x09, 0xfe, 0x03, 0x02, 0x09,
        0xd6, 0x03, 0x04, 0x09, 0xc2, 0x03, 0x06, 0x09,
        0x52, 0x08, 0x09, 0x04, 0x0a, 0x09, 0x02, 0x0c,
        0x09, 0xc8, 0x01, 0x0e, 0x09, 0x00, 0x10, 0x09,
        0x04, 0x05, 0x12, 0x00, 0x02, 0x0e, 0x45, 0x6d,
        0x62, 0x65, 0x67, 0x65, 0x65, 0x02, 0x05, 0x08,
        0x00, 0x09, 0x04, 0x02, 0x07, 0x00, 0x00, 0x53,
        0x32, 0x04, 0x09, 0x02, 0x08, 0x09, 0xf4,
    ];

    #[test]
    fn test_versioned() {
        let mut de = Deserializer::new(HEADER, TYPEINFOS, 18);
        let color: Color = de::Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(color.a, 255);
        assert_eq!(color.r, 235);
        assert_eq!(color.g, 225);
        assert_eq!(color.b, 41);
    }

    #[test]
    fn test_versioned_json() {
        use serde_json;

        let mut de = Deserializer::new(HEADER, TYPEINFOS, 18);
        let color: serde_json::value::Value = de::Deserialize::deserialize(&mut de).unwrap();
        assert_eq!(format!("{:?}", color), "{\"m_a\":255,\"m_b\":41,\"m_g\":225,\"m_r\":235}");
    }
}

