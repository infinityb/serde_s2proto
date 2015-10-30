#![feature(slice_bytes)]

extern crate byteorder;

use std::num::Wrapping;
use std::io::{self, Read, Cursor, Seek, SeekFrom};
use std::sync::{Once, ONCE_INIT};
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian, LittleEndian};

mod enctable;
use self::enctable::ENCRYPTION_TABLE;

static MPQ_FILE_IMPLODE: u32 = 0x00000100;
static MPQ_FILE_COMPRESS: u32 = 0x00000200;
static MPQ_FILE_ENCRYPTED: u32 = 0x00010000;
static MPQ_FILE_FIX_KEY: u32 = 0x00020000;
static MPQ_FILE_SINGLE_UNIT: u32 = 0x01000000;
static MPQ_FILE_DELETE_MARKER: u32 = 0x02000000;
static MPQ_FILE_SECTOR_CRC: u32 = 0x04000000;
static MPQ_FILE_EXISTS: u32 = 0x80000000;

const MPQ_HEADER_FILE_MAGIC: u32 = 0x4d50511a;
const MPQ_HEADER_USER_DATA_MAGIC: u32 = 0x4d50511b;

fn read_header<R: Read>(rdr: &mut R) -> io::Result<MpqHeader> {
    let magic = try!(rdr.read_u32::<BigEndian>());
    Ok(match magic {
        MPQ_HEADER_FILE_MAGIC => {
            let header = try!(MpqFileHeader::from_reader_nomagic(rdr));
            MpqHeader::File(header)
        },
        MPQ_HEADER_USER_DATA_MAGIC => {
            let header = try!(MpqUserDataHeader::from_reader_nomagic(rdr));
            MpqHeader::UserData(header)
        },
        _ => panic!("bad magic")  // FIXME
    })
}

pub struct MpqArchive<R> where R: Read+Seek {
    header_offset: u32,
    header: MpqFileHeader,
    file: R,
}

fn read_exact<R: Read>(rdr: &mut R, buf: &mut [u8]) -> io::Result<()> {
    let to_read = buf.len();
    let mut offset = 0;
    while offset < to_read {
        let new = try!(rdr.read(&mut buf[offset..]));
        if new == 0 {
            break;
        }
        offset += new;
    }
    if offset != to_read {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to fill buffer"))
    }
    Ok(())
}

impl<R> MpqArchive<R> where R: Read+Seek {
    pub fn load(mut file: R) -> io::Result<MpqArchive<R>> where R: Read+Seek {
        Ok(match read_header(&mut file).unwrap() {
            MpqHeader::File(header) => MpqArchive {
                header_offset: 0,
                header: header,
                file: file,
            },
            MpqHeader::UserData(user_header) => {
                try!(file.seek(SeekFrom::Start(user_header.mpq_header_offset as u64)));
                MpqArchive {
                    header_offset: user_header.mpq_header_offset,
                    header: try!(MpqFileHeader::from_reader(&mut file)),
                    file: file,
                }
            }
        })
    }

    fn read_hash_table(&mut self) -> io::Result<Vec<MpqHashTableEntry>> {
        let table_offset = self.header.hash_table_offset + self.header_offset;
        if table_offset != 205652 { panic!("bad offset {}", table_offset) }
        let table_entries = self.header.hash_table_entries;

        let mut buffer = vec![0; 16 * table_entries as usize];
        try!(self.file.seek(SeekFrom::Start(table_offset as u64)));
        try!(read_exact(&mut self.file, &mut buffer));
        let key = string_hash(ENCRYPTION_TABLE, b"(hash table)", StringHashType::Table);
        decrypt(ENCRYPTION_TABLE, key, &mut buffer);

        let mut entry_rdr = Cursor::new(&buffer[..]);
        let mut out = Vec::with_capacity(table_entries as usize);
        for _ in 0..table_entries {
            out.push(try!(MpqHashTableEntry::from_reader(&mut entry_rdr)));
        }
        Ok(out)
    }

    fn read_block_table(&mut self) -> io::Result<Vec<MpqBlockTableEntry>> {
        let table_offset = self.header.block_table_offset + self.header_offset;
        if table_offset != 205908 { panic!("bad offset {}", table_offset) }
        let table_entries = self.header.block_table_entries;

        let mut buffer = vec![0; 16 * table_entries as usize];
        try!(self.file.seek(SeekFrom::Start(table_offset as u64)));
        try!(read_exact(&mut self.file, &mut buffer));
        let key = string_hash(ENCRYPTION_TABLE, b"(block table)", StringHashType::Table);
        decrypt(ENCRYPTION_TABLE, key, &mut buffer);

        let mut entry_rdr = Cursor::new(&buffer[..]);
        let mut out = Vec::with_capacity(table_entries as usize);
        for _ in 0..table_entries {
            out.push(try!(MpqBlockTableEntry::from_reader(&mut entry_rdr)));
        }
        Ok(out)
    }
}

enum MpqHeader {
    File(MpqFileHeader),
    UserData(MpqUserDataHeader),
}


#[repr(C)]
struct MpqFileHeader {
    magic: [u8; 4],
    header_size: u32,
    archive_size: u32,
    format_version: u16,
    sector_size_shift: u16,
    hash_table_offset: u32,
    block_table_offset: u32,
    hash_table_entries: u32,
    block_table_entries: u32,
}

impl MpqFileHeader {
    pub fn magic() -> [u8; 4] {
        [b'M', b'P', b'Q', b'\x1a']
    }

    pub fn from_reader<R: Read>(rdr: &mut R) -> io::Result<MpqFileHeader> {
        let magic = try!(rdr.read_u32::<BigEndian>());
        if MPQ_HEADER_FILE_MAGIC != magic {
            panic!("Bad magic");  // FIXME
        }
        MpqFileHeader::from_reader_nomagic(rdr)
    }

    pub fn from_reader_nomagic<R: Read>(rdr: &mut R) -> io::Result<MpqFileHeader> {
        Ok(MpqFileHeader {
            magic: MpqFileHeader::magic(),
            header_size: try!(rdr.read_u32::<LittleEndian>()),
            archive_size: try!(rdr.read_u32::<LittleEndian>()),
            format_version: try!(rdr.read_u16::<LittleEndian>()),
            sector_size_shift: try!(rdr.read_u16::<LittleEndian>()),
            hash_table_offset: try!(rdr.read_u32::<LittleEndian>()),
            block_table_offset: try!(rdr.read_u32::<LittleEndian>()),
            hash_table_entries: try!(rdr.read_u32::<LittleEndian>()),
            block_table_entries: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

#[repr(C)]
struct MpqUserDataHeader {
    magic: u32,
    user_data_size: u32,
    mpq_header_offset: u32,
    user_data_header_size: u32,
}

impl MpqUserDataHeader {
    fn from_reader<R: Read>(rdr: &mut R) -> io::Result<MpqUserDataHeader> {
        let magic = try!(rdr.read_u32::<BigEndian>());
        if MPQ_HEADER_USER_DATA_MAGIC != magic {
            panic!("Bad magic");  // FIXME
        }
        MpqUserDataHeader::from_reader_nomagic(rdr)
    }

    pub fn from_reader_nomagic<R: Read>(rdr: &mut R) -> io::Result<MpqUserDataHeader> {
        Ok(MpqUserDataHeader {
            magic: MPQ_HEADER_USER_DATA_MAGIC,
            user_data_size: try!(rdr.read_u32::<LittleEndian>()),
            mpq_header_offset: try!(rdr.read_u32::<LittleEndian>()),
            user_data_header_size: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

#[derive(Debug)]
struct MpqHashTableEntry {
    hash_a: u32,
    hash_b: u32,
    locale: u16,
    platform: u16,
    block_table_index: u32,
}

impl MpqHashTableEntry {
    pub fn to_tuple(&self) -> (u32, u32, u16, u16, u32) {
        (self.hash_a, self.hash_b, self.locale, self.platform, self.block_table_index)
    }

    fn from_reader(rdr: &mut Read) -> io::Result<MpqHashTableEntry> {
        Ok(MpqHashTableEntry {
            hash_a: try!(rdr.read_u32::<LittleEndian>()),
            hash_b: try!(rdr.read_u32::<LittleEndian>()),
            locale: try!(rdr.read_u16::<LittleEndian>()),
            platform: try!(rdr.read_u16::<LittleEndian>()),
            block_table_index: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

#[derive(Debug)]
struct MpqBlockTableEntry {
    offset: u32,
    archived_size: u32,
    size: u32,
    flags: u32,
}

impl MpqBlockTableEntry {
    pub fn to_tuple(&self) -> (u32, u32, u32, u32) {
        (self.offset, self.archived_size, self.size, self.flags)
    }

    fn from_reader(rdr: &mut Read) -> io::Result<MpqBlockTableEntry> {
        Ok(MpqBlockTableEntry {
            offset: try!(rdr.read_u32::<LittleEndian>()),
            archived_size: try!(rdr.read_u32::<LittleEndian>()),
            size: try!(rdr.read_u32::<LittleEndian>()),
            flags: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

fn decrypt(table: [Wrapping<u32>; 1280], key: u32, buf: &mut [u8]) {
    // Obviously, this could be optimised and alloc-free

    let mut seed1: Wrapping<u32> = Wrapping(key);
    let mut seed2: Wrapping<u32> = Wrapping(0xEEEEEEEE);

    if buf.len() % 4 != 0 {
        panic!("buffer length must be multiple of four");
    }
    let word_count = buf.len() / 4;
    let mut tmp = Cursor::new(Vec::with_capacity(buf.len()));

    {
        let mut reader = Cursor::new(&buf[..]);
        for _ in 0..word_count {
            let mut value = Wrapping(reader.read_u32::<LittleEndian>().unwrap());
            seed2 = seed2 + table[0x400 + (seed1.0 & 0xFF) as usize];
            value = value ^ (seed1 + seed2);
            seed1 = (((seed1 ^ Wrapping(0xFFFFFFFF)) << 0x15) + Wrapping(0x11111111)) | (seed1 >> 0x0B);
            seed2 = value + seed2 + (seed2 << 5) + Wrapping(3) & Wrapping(0xFFFFFFFF);

            tmp.write_u32::<LittleEndian>(value.0).unwrap();
        }
    }
    let tmp = tmp.into_inner();
    ::std::slice::bytes::copy_memory(&tmp, buf);
}

enum StringHashType {
    TableOffset = 0,
    HashA = 1,
    HashB = 2,
    Table = 3,
}

fn string_hash_normalize(ch: u8) -> Wrapping<u32> {
    if b'a' <= ch && ch <= b'z' {
        Wrapping((ch - b'a' + b'A') as u32)
    } else {
        Wrapping(ch as u32)
    }
}

fn string_hash(table: [Wrapping<u32>; 1280], string: &[u8], hash_type: StringHashType) -> u32 {
    // Please forgive this atrocity
    let hash_tynum: u32 = hash_type as u32;
    let mut seed1 = Wrapping(0x7FED7FED);
    let mut seed2 = Wrapping(0xEEEEEEEE);

    for mut ch in string.iter().cloned().map(string_hash_normalize) {
        let tab_idx = ((hash_tynum << 8) + ch.0) as usize;
        seed1 = table[tab_idx] ^ (seed1 + seed2);
        seed2 = ch + seed1 + seed2 + (seed2 << 5) + Wrapping(3);
    }

    seed1.0
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::{MpqArchive, MpqBlockTableEntry, MpqHashTableEntry};

    static SC2_REPLAY: &'static [u8] = include_bytes!("../../testdata/test.SC2Replay");

    #[test]
    fn test_header_reader() {
        let archive = MpqArchive::load(Cursor::new(SC2_REPLAY)).unwrap();

        assert_eq!(&archive.header.magic, b"MPQ\x1a");
        assert_eq!(archive.header.header_size, 44);
        assert_eq!(archive.header.archive_size, 205044);
        assert_eq!(archive.header.format_version, 1);
        assert_eq!(archive.header.sector_size_shift, 3);
        assert_eq!(archive.header.hash_table_offset, 204628);
        assert_eq!(archive.header.block_table_offset, 204884);
        assert_eq!(archive.header.hash_table_entries, 16);
        assert_eq!(archive.header.block_table_entries, 10);
        assert_eq!(archive.header_offset, 1024);
    }

    #[test]
    fn test_block_table() {
        let mut archive = MpqArchive::load(Cursor::new(SC2_REPLAY)).ok().expect("load fail");
        let entries: Vec<MpqBlockTableEntry> = archive.read_block_table().unwrap();

        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].to_tuple(), (0x0000002C, 727, 890, 0x81000200));
        assert_eq!(entries[1].to_tuple(), (0x00000303, 801, 1257, 0x81000200));
        assert_eq!(entries[2].to_tuple(), (0x00000624, 194096, 479869, 0x81000200));
        assert_eq!(entries[3].to_tuple(), (0x0002FC54, 226, 334, 0x81000200));
        assert_eq!(entries[4].to_tuple(), (0x0002FD36, 97, 97, 0x81000200));
        assert_eq!(entries[5].to_tuple(), (0x0002FD97, 1323, 1970, 0x81000200));
        assert_eq!(entries[6].to_tuple(), (0x000302C2, 6407, 12431, 0x81000200));
        assert_eq!(entries[7].to_tuple(), (0x00031BC9, 533, 2400, 0x81000200));
        assert_eq!(entries[8].to_tuple(), (0x00031DDE, 120, 164, 0x81000200));
        assert_eq!(entries[9].to_tuple(), (0x00031E56, 254, 288, 0x81000200));
    }

    #[test]
    fn test_hash_table() {
        let mut archive = MpqArchive::load(Cursor::new(SC2_REPLAY)).ok().expect("load fail");
        let entries: Vec<MpqHashTableEntry> = archive.read_hash_table().unwrap();

        assert_eq!(entries.len(), 16);
        assert_eq!(entries[ 0].to_tuple(), (0xD38437CB, 0x07DFEAEC, 0x0000, 0x0000, 0x00000009));
        assert_eq!(entries[ 1].to_tuple(), (0xAAC2A54B, 0xF4762B95, 0x0000, 0x0000, 0x00000002));
        assert_eq!(entries[ 2].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[ 3].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[ 4].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[ 5].to_tuple(), (0xC9E5B770, 0x3B18F6B6, 0x0000, 0x0000, 0x00000005));
        assert_eq!(entries[ 6].to_tuple(), (0x343C087B, 0x278E3682, 0x0000, 0x0000, 0x00000004));
        assert_eq!(entries[ 7].to_tuple(), (0x3B2B1EA0, 0xB72EF057, 0x0000, 0x0000, 0x00000006));
        assert_eq!(entries[ 8].to_tuple(), (0x5A7E8BDC, 0xFF253F5C, 0x0000, 0x0000, 0x00000001));
        assert_eq!(entries[ 9].to_tuple(), (0xFD657910, 0x4E9B98A7, 0x0000, 0x0000, 0x00000008));
        assert_eq!(entries[10].to_tuple(), (0xD383C29C, 0xEF402E92, 0x0000, 0x0000, 0x00000000));
        assert_eq!(entries[11].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[12].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[13].to_tuple(), (0xFFFFFFFF, 0xFFFFFFFF, 0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[14].to_tuple(), (0x1DA8B0CF, 0xA2CEFF28, 0x0000, 0x0000, 0x00000007));
        assert_eq!(entries[15].to_tuple(), (0x31952289, 0x6A5FFAA3, 0x0000, 0x0000, 0x00000003));
    }
}
