use std::{io, result};

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
    UnsupportedType,
    InvalidByte,
    InvalidTag(i32),
    TrailingCharacters,
    ExcessiveAllocation,
}

#[derive(Debug)]
pub enum Error {
    SyntaxError(ErrorCode, usize, usize),
    IoError(io::Error),
    MissingFieldError(&'static str),
}

pub type Result<T> = result::Result<T, Error>;

pub struct Deserializer {
    offset: usize,
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
        println!(" $$ read_bits({}) => Ok({}) | {}->{}", bitlen, output, start, end);
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
        let length: u32 = try!(self.parse_vint());
        let start = self.offset / 8;
        self.offset += (length * 8) as usize;
        let end = self.offset / 8;
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

#[cfg(test)]
mod tests {
    use super::super::format::protocol15405::{TYPEINFOS, REPLAY_HEADER_TYPEID};
    use super::{Deserializer};

    const HEADER: &'static [u8] = include_bytes!("../testdata/header");

    // 00000000: 0508 0002 2c53 7461 7243 7261 6674 2049  ....,StarCraft I
    // 00000010: 4920 7265 706c 6179 1b31 3102 050c 0009  I replay.11.....
    // 00000020: 0202 0902 0409 0006 0904 0809 befd 010a  ................
    // 00000030: 09da f001 0409 0406 09b6 8a03            ............
    // {
    //     "m_elapsedGameLoops": 25243, 
    //     "m_signature": "StarCraft II replay\u001b11", 
    //     "m_type": 2, 
    //     "m_version": {
    //         "m_baseBuild": 15405, 
    //         "m_minor": 0, 
    //         "m_revision": 2, 
    //         "m_flags": 1, 
    //         "m_major": 1, 
    //         "m_build": 16223
    //     }
    // }

    #[test]
    fn test() {
        let mut de = Deserializer::new(
            HEADER, TYPEINFOS, REPLAY_HEADER_TYPEID as usize);
        panic!("{:?}", de.root());
    }
}


trait PrimitiveInt: Sized {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()>;
}

        // let mut byte: i64 = try!(self.read_bits(8));
        // let negative = (byte & 1) == 1;
        // let mut result = (byte >> 1) & 0x3F;
        // let mut bits = 6;
        // while (byte & 0x80) != 0 {
        //     byte = try!(self.read_bits(8));
        //     result = result | (byte & 0x7f) << bits;
        //     bits += 7;
        // }
        // let rv = Ok(match negative {
        //     true => -result,
        //     false => result,
        // });
        // rv


impl PrimitiveInt for i32 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(());
        }
        let mut result: i32 = (byte as i32 >> 1) & 0x3F;
        let mut bits = 6;

        while (byte & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;
            
            result = result | (byte as i32 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}

impl PrimitiveInt for i64 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        if (byte & 1) == 1 {
            return Err(());
        }
        let mut result: i64 = (byte as i64 >> 1) & 0x3F;
        let mut bits = 6;

        while (item.1 & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;
            
            result = result | (byte as i64 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}

impl PrimitiveInt for u8 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        let negative = (byte & 1) == 1;
        let mut result: u8 = (byte >> 1) & 0x3F;
        let mut bits = 6;

        while (byte & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;

            result = result | (byte as u8 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}

impl PrimitiveInt for u32 {
    fn from_vint_buf(buffer: &[u8; 20]) -> result::Result<Self, ()> {
        let mut byte_iter = buffer.iter().cloned().enumerate();

        let mut item = byte_iter.next().unwrap();
        let (_, byte) = item;

        let negative = (byte & 1) == 1;
        let mut result: u32 = (byte as u32 >> 1) & 0x3F;
        let mut bits = 6;

        while (byte & 0x80) != 0 && item.0 < buffer.len() {
            item = byte_iter.next().unwrap();
            let (_, byte) = item;

            result = result | (byte as u32 & 0x7f) << bits;
            bits += 7;
        }

        Ok(result)
    }
}