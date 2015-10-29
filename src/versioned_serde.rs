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
    InvalidByte,
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
        // panic!("error str: {}", err_str);
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
    offset: usize, // in bits
    buffer: Vec<u8>,
    typeinfos: &'static [TypeInfo],
    root_typeinfo: usize,
}

impl Deserializer {
    pub fn new(buf: &[u8], typeinfos: &'static [TypeInfo], root_typeinfo: usize) -> Deserializer {
        Deserializer {
            offset: 0,
            buffer: buf.to_vec(),
            typeinfos: typeinfos,
            root_typeinfo: root_typeinfo,
        }
    }

    // TODO: generic over output ints
    fn read_bits(&mut self, bitlen: u8) -> Result<i64> {
        if bitlen % 8 != 0 {
            unimplemented!();
        }
        if self.offset % 8 != 0 {
            unimplemented!();
        }
        let start = self.offset / 8;
        let end = start + (bitlen / 8) as usize;
        self.offset += bitlen as usize;

        if self.buffer.len() < end{
            return Err(Error::SyntaxError(ErrorCode::UnexpectedEOF, 0, 0));
        }

        let mut output: i64 = 0;
        for i in start..end {
            output = output << 8;
            output += self.buffer[i] as i64;
        }
        Ok(output)
    }

    fn expect_skip(&mut self, expected: u8) -> Result<()> {
        if self.offset % 8 != 0 {
            unimplemented!();
        }
        if try!(self.read_bits(8)) == expected as i64 {
            Ok(())
        } else {
            Err(Error::SyntaxError(ErrorCode::InvalidByte, 0, 0))
        }
    }

    // TODO: generic over output ints
    fn parse_vint<T>(&mut self) -> Result<T> where T: PrimitiveInt {
        let mut buffer: [u8; 20] = [0; 20];
        for byte in buffer.iter_mut() {
            *byte = try!(self.read_bits(8)) as u8;
            if (*byte & 0x80) == 0 {
                break;
            }
        }

        // FIXME: error
        T::from_vint_buf(&buffer)
            .map_err(|_| Error::SyntaxError(ErrorCode::InvalidByte, 0, 0))
    }

    pub fn root(&mut self) -> Result<Vec<String>> {
        let rt = self.root_typeinfo as u32;
        self.instance(rt)
    }

    pub fn instance(&mut self, typeid: TypeId) -> Result<Vec<String>> {
        match self.typeinfos[typeid as usize] {
            TypeInfo::Struct(st) => self.struct_(st),
            TypeInfo::Int { bounds } => self.int(bounds),
            TypeInfo::Blob { len } => self.blob(len),
            _ => panic!(),
        }
    }

    pub fn blob(&mut self, len: IntBounds) -> Result<Vec<String>> {
        try!(self.expect_skip(2));
        let length = try!(self.parse_vint::<u32>()) as usize;
        let start = self.offset / 8;
        self.offset += length * 8;
        let end = start + length;
        Ok(vec![
            format!("{:?}", &self.buffer[start..end])
        ])
    }

    pub fn int(&mut self, bounds: IntBounds) -> Result<Vec<String>> {
        try!(self.expect_skip(9));
        let value = bounds.min + try!(self.parse_vint::<i64>());
        Ok(vec![format!("{}", value)])
    }

    pub fn struct_(&mut self, st: Struct) -> Result<Vec<String>> {
        try!(self.expect_skip(5));
        let length: u32 = try!(self.parse_vint());
        println!("reading struct of len={}: {:?}", length, st);

        let mut output = Vec::new();
        for i in 0..length {
            let tag: i32 = try!(self.parse_vint());
            println!("   tag={} @ idx={}", tag, i);
            let invalid = Error::SyntaxError(ErrorCode::InvalidTag(tag), 0, 0);
            let field_info = try!(find_struct_field(st, tag).ok_or(invalid));
            let child = try!(self.instance(field_info.1));
            let emit = format!("{:?} => {:?}", field_info.0, child);
            println!("EMIT {}", emit);
            output.push(emit);
        }
        return Ok(output);

        fn find_struct_field(st: Struct, tag: i32) -> Option<StructField> {
            for field in st.fields.iter() {
                if field.2 == tag {
                    return Some(*field)
                }
            }
            None
        }
    }
}

impl serde::Deserializer for Deserializer {
    type Error = Error;

    #[inline]
    fn visit<V>(&mut self, mut visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor,
    {
        let typeid = try!(self.read_bits(8)) as u8;
        match typeid {
            0x02 => {
                let length = try!(self.parse_vint::<u32>()) as usize;
                let start = self.offset / 8;
                self.offset += length * 8;
                let end = start + length;
                visitor.visit_bytes(&self.buffer[start..end])
            },
            0x05 => {
                let struct_def = match self.typeinfos[self.root_typeinfo] {
                    TypeInfo::Struct(sd) => sd,
                    _ => return Err(Error::SyntaxError(ErrorCode::UnexpectedType, 0, 0)),
                };
                let length = try!(self.parse_vint::<u32>()) as usize;
                let root_ti = self.root_typeinfo;
                let x = visitor.visit_map(StructVisitor::new(self, struct_def, root_ti, length));

                println!("lol x.is_ok() = {:?}", x.is_ok());
                x
            },
            0x09 => {
                // TODO: decide best number type to visit here
                visitor.visit_u64(try!(self.parse_vint()))
            },
            _ => return Err(Error::SyntaxError(ErrorCode::UnsupportedType(typeid), 0, 0)),
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

enum StructVisitorState {
    KeyNext,
    ValueNext,
}

struct StructVisitor<'a> {
    de: &'a mut Deserializer,
    struct_def: Struct,
    typeinfo_idx: usize,
    length: usize,
    offset: usize,
    state: StructVisitorState,
}

impl<'a> StructVisitor<'a> {
    fn new(de: &'a mut Deserializer, struct_def: Struct, ti_idx: usize, length: usize) -> Self {
        StructVisitor {
            de: de,
            struct_def: struct_def,
            typeinfo_idx: ti_idx,
            length: length,
            offset: 0,
            state: StructVisitorState::KeyNext,
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
        println!("------------");
        match try!(self.visit_key()) {
            Some(key) => {
                let value = try!(self.visit_value());
                println!("StructVisitor emits");
                ::quux();
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
        println!("StructVisitor visit_key() :: offset = {:?}", self.de.offset);
        match self.state {
            StructVisitorState::KeyNext => {
                self.state = StructVisitorState::ValueNext;
            },
            StructVisitorState::ValueNext => {
                return Err(serde::de::Error::unknown_field(""));
            },
        }
        let tag: i32 = try!(self.de.parse_vint());
        println!("StructVisitor visit_key() :2: offset = {:?}", self.de.offset);

        de::Deserialize::deserialize(&mut TagDeserializer {
            tag: tag,
            struct_def: self.struct_def,
        }).map(Some)
    }

    fn visit_value<V>(&mut self) -> Result<V>
        where V: de::Deserialize,
    {
        println!("StructVisitor visit_value() :: offset = {:?}", self.de.offset);
        match self.state {
            StructVisitorState::KeyNext => {
                return Err(serde::de::Error::unknown_field(""));
            },
            StructVisitorState::ValueNext => {
                self.state = StructVisitorState::KeyNext;
            },
        }
        let sidx = self.de.offset / 8;
        let mut deserializer = Deserializer::new(
            &self.de.buffer[sidx..],
            self.de.typeinfos,
            self.typeinfo_idx);
        let rv = de::Deserialize::deserialize(&mut deserializer);
        self.de.offset += deserializer.offset;
        self.offset += 1;
        rv
    }

    fn end(&mut self) -> Result<()> {
        if self.length != self.offset {
            println!("StructVisitor: erroneous end");
            return Err(serde::de::Error::unknown_field(""));
        }
        println!("StructVisitor ended");
        Ok(())
    }

    fn missing_field<V>(&mut self, _field: &'static str) -> Result<V>
        where V: de::Deserialize,
    {
        let mut de = de::value::ValueDeserializer::into_deserializer(());
        Ok(try!(de::Deserialize::deserialize(&mut de)))
    }
}

pub struct TagDeserializer {
    tag: i32,
    struct_def: Struct,
}

impl de::Deserializer for TagDeserializer {
    type Error = Error;

    #[inline]
    fn visit<V>(&mut self, mut visitor: V) -> Result<V::Value> where V: de::Visitor {
        for &(name, _, tag) in self.struct_def.fields.iter() {
            if tag == self.tag {
                return visitor.visit_str(name);
            }
        }
        panic!("INVALID TAG {} ON {:?}", self.tag, self.struct_def);
    }

    fn visit_str<V>(&mut self, visitor: V) -> Result<V::Value> where V: de::Visitor {
        panic!("TagDeserializer::visit_str");
    }

    fn visit_string<V>(&mut self, mut visitor: V) -> Result<V::Value> where V: de::Visitor {
        ::quux();
        for &(name, _, tag) in self.struct_def.fields.iter() {
            if tag == self.tag {
                return visitor.visit_str(name);
            }
        }
        panic!("INVALID TAG {} ON {:?}", self.tag, self.struct_def);
    }
}

// pub struct TagVisitor {
//     tag: i32,
//     struct_def: Struct,
// }

// pub struct TypeInfoNameVisitor {
//     ti: &'static TypeInfo,
// }

// impl Visitor for TypeInfoNameVisitor {
//     type Value = &'str;

//     #[inline]
//     fn visit_i32<E>(&mut self, v: i64) -> Result<Self::Value, E> where E: de::Error {
//         for field in self.ti.0.fields.iter() {
//             if field.3 == v {
//                 return Ok(TypeInfoName(field.0));
//             }
//         }
//         Err(de::Error::unknown_field(""))
//     }
// }

// pub struct TypeInfoName(pub &'static str);

// impl Deserialize for TypeInfoName {
//     fn deserialize<D>(deserializer: &mut D) -> Result<String, D::Error>
//         where D: Deserializer,
//     {   
//         deserializer.visit_i32(TypeInfoNameVisitor {
//             ti: ????,
//         })
//     }
// }

// struct TypeInfoTagDeserializer {
//     de: &'a mut Deserializer,
//     ti: &'static TypeInfo,
// }

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
            println!("item = {:?}, item.1 & 0x80 = {:?}", item, item.1 & 0x80);
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
        println!("read byte = {:?}", byte);

        if (byte & 1) == 1 {
            return Err(())
        }
        let mut result: u64 = (byte as u64 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;
            println!("read byte = {:?}", byte);
            println!("bits = {}", bits);
            result = result | ((byte as u64 & 0x7f) << bits);
            bits += 7;
        }
        if result > 0xFF {
            println!("WARN: overflow!!");
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