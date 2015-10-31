use std::collections::BTreeMap;

use serde::{ser, de};
use serde::bytes::ByteBuf;

#[derive(PartialEq, Eq, Debug)]
pub enum Value {
    I64(i64),
    U64(u64),
    Bytes(Vec<u8>),
    String(String),
    Array(Vec<Value>),
    Dict(BTreeMap<String, Value>),
    Optional(Box<Value>),
    Boolean(bool),
    Null,
}


impl Value {
    pub fn as_str(&self) -> Result<&str, ()> {
        match *self {
            Value::String(ref val) => Ok(&*val),
            _ => return Err(())
        }
    }

    pub fn as_array(&self) -> Result<&[Value], ()> {
        match *self {
            Value::Array(ref val) => Ok(&*val),
            Value::Optional(ref val) => val.as_array(),
            _ => return Err(())
        }
    }

    pub fn as_u64(&self) -> Result<u64, ()> {
        match *self {
            Value::U64(val) => Ok(val),
            _ => return Err(())
        }
    }

    pub fn as_i64(&self) -> Result<i64, ()> {
        match *self {
            Value::I64(val) => Ok(val),
            _ => return Err(())
        }
    }

    pub fn get_path(&self, path: &[&str]) -> Result<&Value, ()> {
        let mut root: &Value = self;
        for &part in path.iter() {
            match *root {
                Value::Dict(ref map) => {
                    root = try!(map.get(part).ok_or(()));
                },
                _ => return Err(())
            }
        }
        Ok(root)
    }
}

impl de::Deserialize for Value {
    #[inline]
    fn deserialize<D>(deserializer: &mut D) -> Result<Value, D::Error>
        where D: de::Deserializer,
    {
        struct ValueVisitor;

        impl de::Visitor for ValueVisitor {
            type Value = Value;

            #[inline]
            fn visit_bool<E>(&mut self, value: bool) -> Result<Self::Value, E> {
                Ok(Value::Boolean(value))
            }

            #[inline]
            fn visit_u8<E>(&mut self, value: u8) -> Result<Value, E> {
                Ok(Value::U64(value as u64))
            }

            #[inline]
            fn visit_u16<E>(&mut self, value: u16) -> Result<Value, E> {
                Ok(Value::U64(value as u64))
            }

            #[inline]
            fn visit_u32<E>(&mut self, value: u32) -> Result<Value, E> {
                Ok(Value::U64(value as u64))
            }

            #[inline]
            fn visit_u64<E>(&mut self, value: u64) -> Result<Value, E> {
                Ok(Value::U64(value))
            }

            #[inline]
            fn visit_i8<E>(&mut self, value: i8) -> Result<Value, E> {
                Ok(Value::I64(value as i64))
            }

            #[inline]
            fn visit_i16<E>(&mut self, value: i16) -> Result<Value, E> {
                Ok(Value::I64(value as i64))
            }

            #[inline]
            fn visit_i32<E>(&mut self, value: i32) -> Result<Value, E> {
                Ok(Value::I64(value as i64))
            }

            #[inline]
            fn visit_i64<E>(&mut self, value: i64) -> Result<Value, E> {
                Ok(Value::I64(value))
            }

            #[inline]
            fn visit_seq<V>(&mut self, visitor: V) -> Result<Value, V::Error>
                where V: de::SeqVisitor,
            {
                let values = try!(de::impls::VecVisitor::new().visit_seq(visitor));
                Ok(Value::Array(values))
            }

            #[inline]
            fn visit_map<V>(&mut self, visitor: V) -> Result<Value, V::Error>
                where V: de::MapVisitor,
            {
                let values: BTreeMap<String, Value> = try!(
                    de::impls::BTreeMapVisitor::new().visit_map(visitor));
                Ok(Value::Dict(values))
            }

            #[inline]
            fn visit_str<E>(&mut self, value: &str) -> Result<Self::Value, E> {
                Ok(Value::String(From::from(value)))
            }

            #[inline]
            fn visit_string<E>(&mut self, value: String) -> Result<Self::Value, E> {
                Ok(Value::String(value))
            }

            #[inline]
            fn visit_bytes<E>(&mut self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(Value::Bytes(value.to_vec()))
            }

            #[inline]
            fn visit_byte_buf<E>(&mut self, value: Vec<u8>) -> Result<Self::Value, E> {
                Ok(Value::Bytes(value))
            }

            fn visit_none<E>(&mut self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            #[inline]
            fn visit_some<D>(&mut self, des: &mut D) -> Result<Self::Value, D::Error>
                where D: de::Deserializer
            {
                de::Deserialize::deserialize(des).map(Value::Optional)
            }


        }

        deserializer.visit(ValueVisitor)
    }
}