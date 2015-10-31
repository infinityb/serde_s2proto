use std::{io, result};

use serde;
use serde::de;

use super::format::{
    TypeInfo,
    TypeId,
    Struct,
    StructField,
    IntBounds,
};

#[derive(Debug)]
pub enum ErrorCode {
    UnexpectedEOF,
    ExpectedSomeValue,
    KeyMustBeABytes,    
    UnsupportedType(u8),
    UnexpectedType,
    InvalidByte(u8),
    InvalidTag(i32),
    TrailingCharacters,
    ExcessiveAllocation,
    Unknown,
}

#[derive(Debug)]
pub enum Error {
    SyntaxError(ErrorCode, usize, usize),
    IoError(io::Error),
    UnknownFieldError,
    MissingFieldError(&'static str),
}

impl serde::de::Error for Error {
    fn syntax(err_str: &str) -> Error {
        Error::SyntaxError(ErrorCode::Unknown, 0, 0)
    }

    fn end_of_stream() -> Error {
        Error::SyntaxError(ErrorCode::UnexpectedEOF, 0, 0)
    }

    fn unknown_field(field: &str) -> Error {
        Error::UnknownFieldError
    }

    fn missing_field(field: &'static str) -> Error {
        Error::MissingFieldError(field)
    }
}

impl From<serde::de::value::Error> for Error {
    fn from(e: serde::de::value::Error) -> Error {
        use serde::de::value::Error as SerdeErr;
        match e {
            SerdeErr::SyntaxError => Error::SyntaxError(ErrorCode::Unknown, 0, 0),
            SerdeErr::EndOfStreamError => Error::SyntaxError(ErrorCode::UnexpectedEOF, 0, 0),
            SerdeErr::UnknownFieldError(_) => Error::UnknownFieldError,
            SerdeErr::MissingFieldError(v) => Error::MissingFieldError(v),

        }
    }
}

pub type Result<T> = result::Result<T, Error>;

pub struct Deserializer {
    offset: usize,
    buffer: Vec<u8>,
    typeinfos: &'static [TypeInfo],
    typestack: Vec<&'static TypeInfo>,
    // root_typeinfo: usize,
}

impl Deserializer {
    pub fn new(buf: &[u8], typeinfos: &'static [TypeInfo], root_typeinfo: usize) -> Deserializer {
        Deserializer {
            offset: 0,
            buffer: buf.to_vec(),
            typeinfos: typeinfos,
            typestack: vec![&typeinfos[root_typeinfo]],
        }
    }

    fn read_byte(&mut self) -> Result<u8> {
        if self.buffer.len() <= self.offset {
            return Err(Error::SyntaxError(ErrorCode::UnexpectedEOF, 0, 0));
        }
        let byte = self.buffer[self.offset];
        self.offset += 1;
        Ok(byte)
    }

    fn expect_skip(&mut self, expected: u8) -> Result<()> {
        let got = try!(self.read_byte());
        if got == expected {
            Ok(())
        } else {
            Err(Error::SyntaxError(
                ErrorCode::InvalidByte(got),
                self.offset, self.offset + 1))
        }
    }

    fn parse_vint<T>(&mut self) -> Result<T> where T: PrimitiveInt {
        let pre_offset = self.offset;
        let mut buffer: [u8; 20] = [0; 20];
        for byte in buffer.iter_mut() {
            *byte = try!(self.read_byte());
            if (*byte & 0x80) == 0 {
                break;
            }
        }

        let err = Error::SyntaxError(ErrorCode::Unknown, pre_offset, self.offset);
        T::from_vint_buf(&buffer).map_err(|_| err)
    }

    fn top_typeinfo(&self) -> Result<&'static TypeInfo> {
        // TODO: make not panic
        let last_ti: &'static TypeInfo = *self.typestack.last().unwrap();

        Ok(last_ti)
    }
}

impl serde::de::Deserializer for Deserializer {
    type Error = Error;

    #[inline]
    fn visit<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        let pre_offset = self.offset;
        let typeid = try!(self.read_byte());
        let no_support = Error::SyntaxError(ErrorCode::UnsupportedType(typeid), 0, 0);
        match typeid {
            0x00 => {
                // array
                let length = try!(self.parse_vint::<u32>()) as usize;
                let start = self.offset;

                let opt_typinfo = try!(self.top_typeinfo());
                let typeid = match *opt_typinfo {
                    TypeInfo::Array { typeid, .. } => typeid,
                    _ => return Err(Error::SyntaxError(ErrorCode::UnexpectedType, 0, 0)),
                };
                let typeinfo: &'static TypeInfo = &self.typeinfos[typeid as usize];
                visitor.visit_seq(ArrayVisitor::new(self, length, typeinfo))
            }
            0x01 => {
                // bitarray
                Err(no_support)
            }
            0x02 => {
                // blob
                let length = try!(self.parse_vint::<u32>()) as usize;
                let start = self.offset;
                self.offset += length;
                let buf = &self.buffer[start..self.offset];
                match ::std::str::from_utf8(buf) {
                    Ok(str_val) => visitor.visit_str(str_val),
                    Err(_) => {
                        let res0: Result<V::Value> = visitor.visit_bytes(buf);
                        res0.or_else(|_| visitor.visit_string(format!("{:?}", buf)))
                    }
                }
            },
            0x03 => {
                // choice aka enum
                Err(no_support)
            },
            0x04 => {
                // optional
                let is_some = try!(self.read_byte()) != 0;
                if !is_some {
                    return visitor.visit_none();
                }

                let opt_typinfo = try!(self.top_typeinfo());
                let typeid = match *opt_typinfo {
                    TypeInfo::Optional { typeid } => typeid,
                    _ => return Err(Error::SyntaxError(ErrorCode::UnexpectedType, 0, 0)),
                };

                let typeinfo: &'static TypeInfo = &self.typeinfos[typeid as usize];
                self.typestack.push(typeinfo);
                let result = visitor.visit_some(self);
                assert_eq!(self.typestack.pop().unwrap() as *const TypeInfo, typeinfo as *const TypeInfo);
                result
            }
            0x05 => {
                let length = try!(self.parse_vint::<u32>()) as usize;
                visitor.visit_map(StructVisitor::new(self, length))
            },
            0x06 => {
                let boolbyte = try!(self.read_byte());
                visitor.visit_bool(boolbyte != 0)
            },
            0x07 => {
                // FourCC or real32
                let start = self.offset;
                self.offset += 4;
                let buf = &self.buffer[start..self.offset];

                let buffer = format!("{:?}", buf);
                visitor.visit_string(buffer)
            },
            0x08 => {
                // real64
                Err(no_support)
            },
            0x09 => {
                // variable-length integer
                // TODO: decide best number type to visit here
                let val = try!(self.parse_vint());
                visitor.visit_u64(val)
            },
            _ => Err(no_support),
        }
    }

    fn visit_bool<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        unimplemented!();
    }

    #[inline]
    fn visit_u8<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_u16<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_u32<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_u64<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_usize<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_i8<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_i16<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_i32<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_i64<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    #[inline]
    fn visit_isize<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        try!(self.expect_skip(9));
        visitor.visit_u8(try!(self.parse_vint()))
    }

    fn visit_tuple<V>(&mut self,
                      len: usize,
                      mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        struct TupleVisitor<'a>(&'a mut Deserializer);

        impl<'a> serde::de::SeqVisitor for TupleVisitor<'a> {
            type Error = Error;

            fn visit<T>(&mut self) -> result::Result<Option<T>, Self::Error>
                where T: serde::de::Deserialize,
            {
                let value = try!(serde::Deserialize::deserialize(self.0));
                Ok(Some(value))
            }

            fn end(&mut self) -> result::Result<(), Self::Error> {
                Ok(())
            }
        }

        try!(self.expect_skip(5));
        let length: usize = try!(self.parse_vint());
        assert_eq!(length, len);
        visitor.visit_seq(TupleVisitor(self))
    }
}

#[derive(Copy, Clone)]
enum StructVisitorState {
    KeyNext,
    ValueNext(&'static TypeInfo),
}

impl PartialEq for StructVisitorState {
    fn eq(&self, other: &StructVisitorState) -> bool {
        use self::StructVisitorState as SVS;
        match (*self, *other) {
            (SVS::KeyNext, SVS::KeyNext) => true,
            (SVS::ValueNext(sti), SVS::ValueNext(oti)) => {
                (sti as *const TypeInfo) == (oti as *const TypeInfo)
            },
            _ => false,
        }
    }
}

impl Eq for StructVisitorState {}

struct StructVisitor<'a> {
    de: &'a mut Deserializer,
    length: usize,
    offset: usize,
    state: StructVisitorState,
}

impl<'a> StructVisitor<'a> {
    fn new(de: &'a mut Deserializer, length: usize) -> Self {
        StructVisitor {
            de: de,
            length: length,
            offset: 0,
            state: StructVisitorState::KeyNext,
        }
    }

    fn struct_def(&self) -> Result<Struct> {
        match **self.de.typestack.last().unwrap() {
            TypeInfo::Struct(ref st) => Ok(*st),
            _ => Err(Error::SyntaxError(ErrorCode::UnexpectedType, 0, 0)),
        }
    }

    fn expect_state_key(&self) -> Result<()> {
        match self.state {
            StructVisitorState::KeyNext => Ok(()),
            StructVisitorState::ValueNext(_) => {
                panic!("internal failure");
            }
        }
    }

    fn expect_state_value(&self) -> Result<&'static TypeInfo> {
        match self.state {
            StructVisitorState::KeyNext => {
                panic!("internal failure");
            },
            StructVisitorState::ValueNext(ti) => Ok(ti)
        }
    }
}

impl<'a> de::MapVisitor for StructVisitor<'a> {
    type Error = Error;

    #[inline]
    fn visit<K, V>(&mut self) -> Result<Option<(K, V)>>
        where K: de::Deserialize,
              V: de::Deserialize,
    {
        match try!(self.visit_key()) {
            Some(key) => {
                let value = try!(self.visit_value());
                Ok(Some((key, value)))
            }
            None => Ok(None)
        }
    }

    fn visit_key<K>(&mut self) -> Result<Option<K>>
        where K: de::Deserialize,
    {
        if self.length == self.offset {
            return Ok(None);
        }
        try!(self.expect_state_key());
        let need_tag: i32 = try!(self.de.parse_vint());
        let struct_def = try!(self.struct_def());

        for &(name, next_type, tag) in struct_def.fields.iter() {
            if need_tag == tag {
                let next_typeinfo: &'static TypeInfo = &self.de.typeinfos[next_type as usize];
                self.state = StructVisitorState::ValueNext(next_typeinfo);
                return de::Deserialize::deserialize(&mut StrVisitor(name)).map(Some)
            }
        }
        Err(Error::SyntaxError(ErrorCode::InvalidTag(need_tag), 0, 0))
    }

    fn visit_value<V>(&mut self) -> Result<V>
        where V: de::Deserialize,
    {
        let typeinfo = try!(self.expect_state_value());
        self.state = StructVisitorState::KeyNext;
        self.de.typestack.push(typeinfo);
        self.offset += 1;
        let rv = de::Deserialize::deserialize(self.de);
        self.de.typestack.pop().unwrap();
        rv
    }

    fn end(&mut self) -> Result<()> {
        if self.length != self.offset {
            panic!("internal error: bad number of values iterated");
        }
        Ok(())
    }

    fn missing_field<V>(&mut self, _field: &'static str) -> Result<V>
        where V: de::Deserialize,
    {
        let mut de = de::value::ValueDeserializer::into_deserializer(());
        Ok(try!(de::Deserialize::deserialize(&mut de)))
    }
}

/// 
struct ArrayVisitor<'a> {
    de: &'a mut Deserializer,
    length: usize,
    offset: usize,
    item_ti: &'static TypeInfo,
}

impl<'a> ArrayVisitor<'a> {
    fn new(de: &'a mut Deserializer, length: usize, item_ti: &'static TypeInfo) -> Self {
        ArrayVisitor {
            de: de,
            length: length,
            offset: 0,
            item_ti: item_ti,
        }
    }
}

impl<'a> de::SeqVisitor for ArrayVisitor<'a> {
    type Error = Error;

    fn visit<T>(&mut self) -> Result<Option<T>> where T: de::Deserialize {
        if self.length == self.offset {
            return Ok(None);
        }
        self.de.typestack.push(self.item_ti);
        let rv = de::Deserialize::deserialize(self.de);
        assert_eq!(self.de.typestack.pop().unwrap() as *const TypeInfo, self.item_ti as *const TypeInfo);
        self.offset += 1;
        rv.map(Some)
    }

    fn end(&mut self) -> Result<()> {
        if self.length != self.offset {
            panic!("internal error: bad number of values iterated");
        }
        Ok(())
    }
}

/// just visits a string
struct StrVisitor<'a>(&'a str);

impl<'a> de::Deserializer for StrVisitor<'a> {
    type Error = Error;

    #[inline]
    fn visit<V>(&mut self, mut visitor: V) -> Result<V::Value> where V: de::Visitor {
        let res0: Result<V::Value> = visitor.visit_str(self.0);
        res0.or_else(|_| visitor.visit_bytes(self.0.as_bytes()))
    }

    fn visit_str<V>(&mut self, mut visitor: V) -> Result<V::Value> where V: de::Visitor {
        let rv = visitor.visit_str(self.0);
        rv
    }

    fn visit_string<V>(&mut self, mut visitor: V) -> Result<V::Value> where V: de::Visitor {
        let rv = visitor.visit_str(self.0);
        rv
    }
}


trait PrimitiveInt: Sized {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()>;
}

impl PrimitiveInt for i32 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();

        let negate = (item.1 & 1) == 1;
        let mut result: i32 = (item.1 as i32 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            result = result | (item.1 as i32 & 0x7f) << bits;
            bits += 7;
        }

        Ok(match negate {
            true => -result,
            false => result,
        })
    }
}

impl PrimitiveInt for i64 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        let negate = (byte & 1) == 1;
        let mut result: i64 = (byte as i64 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;
            
            result = result | (byte as i64 & 0x7f) << bits;
            bits += 7;
        }

        Ok(match negate {
            true => -result,
            false => result,
        })
    }
}

impl PrimitiveInt for u8 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(())
        }
        let mut result: u64 = (byte as u64 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;
            result = result | ((byte as u64 & 0x7f) << bits);
            bits += 7;
        }
        if result > 0xFF {
            panic!("BUG?: overflow!!");
        }

        Ok((result & 0xFF) as u8)
    }
}

impl PrimitiveInt for u32 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(())
        }
        let mut result: u32 = (byte as u32 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;

            result = result | (byte as u32 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}

impl PrimitiveInt for u64 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(())
        }
        let mut result: u64 = (byte as u64 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;

            result = result | (byte as u64 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}


impl PrimitiveInt for usize {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(())
        }
        let mut result: usize = (byte as usize >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;

            result = result | (byte as usize & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}