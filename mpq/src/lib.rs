#![feature(slice_bytes)]

extern crate byteorder;
extern crate bzip2;

use std::num::Wrapping;
use std::io::{self, Read, Cursor, Seek, SeekFrom};
use std::sync::{Once, ONCE_INIT};
use std::collections::HashMap;

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

fn read_header<R: Read>(rdr: &mut R) -> io::Result<Header> {
    let magic = try!(rdr.read_u32::<BigEndian>());
    Ok(match magic {
        MPQ_HEADER_FILE_MAGIC => {
            let header = try!(FileHeader::from_reader_nomagic(rdr));
            Header::File(header)
        },
        MPQ_HEADER_USER_DATA_MAGIC => {
            let header = try!(UserDataHeader::from_reader_nomagic(rdr));
            Header::UserData(header)
        },
        _ => panic!("bad magic")  // FIXME
    })
}

pub struct Archive<R> where R: Read+Seek {
    header_offset: u32,
    header: FileHeader,
    hash_table: HashMap<HashTableKey, HashTableValue>,
    block_table: Vec<BlockTableEntry>,
    file: R,
}

impl<R> Archive<R> where R: Read+Seek {
    pub fn load(mut file: R) -> io::Result<Archive<R>> where R: Read+Seek {
        let (header_off, header) = match read_header(&mut file).unwrap() {
            Header::File(header) => (0, header),
            Header::UserData(user_header) => {
                try!(file.seek(SeekFrom::Start(user_header.mpq_header_offset as u64)));
                let header = try!(FileHeader::from_reader(&mut file));
                (user_header.mpq_header_offset, header)
            }
        };
        let hash_table = try!(read_hash_table(&mut file, &header, header_off));
        let block_table = try!(read_block_table(&mut file, &header, header_off));
        Ok(Archive {
            header_offset: header_off,
            header: header,
            hash_table: hash_table,
            block_table: block_table,
            file: file,
        })
    }

    pub fn read_file(&mut self, filename: &[u8], into: &mut Vec<u8>) -> io::Result<usize> {
        let hash_a = string_hash(ENCRYPTION_TABLE, filename, StringHashType::HashA);
        let hash_b = string_hash(ENCRYPTION_TABLE, filename, StringHashType::HashB);
        let hash_entry = try!(self.hash_table
            .get(&(hash_a, hash_b))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found")));
        let block_entry = try!(self.block_table
            .get(hash_entry.block_table_index as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found")));

        let mut file_data = Vec::new();
        let data_offset = (self.header_offset + block_entry.offset) as u64;
        try!(self.file.seek(SeekFrom::Start(data_offset)));
        {
            let mut data_reader = self.file.by_ref().take(block_entry.archived_size as u64);
            let bytes_read = try!(data_reader.read_to_end(&mut file_data));
            if bytes_read < block_entry.archived_size as usize {
                return Err(io::Error::new(io::ErrorKind::Other, "file truncated"));
            }
        }

        if (block_entry.flags & MPQ_FILE_EXISTS) == 0 {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        }
        if (block_entry.flags & MPQ_FILE_ENCRYPTED) > 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "encrypted files not supported"));
        }
        if block_entry.archived_size == 0 {
            return Ok(0);
        }

        let has_crc = (block_entry.flags & MPQ_FILE_SECTOR_CRC) > 0;
        if (block_entry.flags & MPQ_FILE_SINGLE_UNIT) == 0 {
            // multi-unit file
            let sector_size = 512 << self.header.sector_size_shift;

            let mut sectors = block_entry.size / sector_size + 1;
            if has_crc {
                sectors += 1;
            }
            let mut positions = Vec::new();
            for _ in 0..sectors {
                positions.push(try!(self.file.read_u32::<LittleEndian>()));
            }

            // unimplemented;
            return Err(io::Error::new(io::ErrorKind::Other, "multi-unit files not supported yet"));
        } else {
            let is_compressed = {
                ((block_entry.flags & MPQ_FILE_COMPRESS) > 0) &&
                block_entry.size > block_entry.archived_size
            };
            return if is_compressed {
                decompress(&file_data, into).map_err(|_| {
                    io::Error::new(io::ErrorKind::Other, "error decompressing file")
                })
            } else {
                let length = file_data.len();
                into.extend(file_data.into_iter());
                Ok(length)
            }   
        }

        unimplemented!()
    }
}

enum Header {
    File(FileHeader),
    UserData(UserDataHeader),
}


#[repr(C)]
struct FileHeader {
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

impl FileHeader {
    pub fn magic() -> [u8; 4] {
        [b'M', b'P', b'Q', b'\x1a']
    }

    pub fn from_reader<R: Read>(rdr: &mut R) -> io::Result<FileHeader> {
        let magic = try!(rdr.read_u32::<BigEndian>());
        if MPQ_HEADER_FILE_MAGIC != magic {
            panic!("Bad magic");  // FIXME
        }
        FileHeader::from_reader_nomagic(rdr)
    }

    pub fn from_reader_nomagic<R: Read>(rdr: &mut R) -> io::Result<FileHeader> {
        Ok(FileHeader {
            magic: FileHeader::magic(),
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
struct UserDataHeader {
    magic: u32,
    user_data_size: u32,
    mpq_header_offset: u32,
    user_data_header_size: u32,
}

impl UserDataHeader {
    fn from_reader<R: Read>(rdr: &mut R) -> io::Result<UserDataHeader> {
        let magic = try!(rdr.read_u32::<BigEndian>());
        if MPQ_HEADER_USER_DATA_MAGIC != magic {
            panic!("Bad magic");  // FIXME
        }
        UserDataHeader::from_reader_nomagic(rdr)
    }

    pub fn from_reader_nomagic<R: Read>(rdr: &mut R) -> io::Result<UserDataHeader> {
        Ok(UserDataHeader {
            magic: MPQ_HEADER_USER_DATA_MAGIC,
            user_data_size: try!(rdr.read_u32::<LittleEndian>()),
            mpq_header_offset: try!(rdr.read_u32::<LittleEndian>()),
            user_data_header_size: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

type HashTableKey = (u32, u32);

#[derive(Debug)]
struct HashTableValue {
    locale: u16,
    platform: u16,
    block_table_index: u32,
}

impl HashTableValue {
    pub fn to_tuple(&self) -> (u16, u16, u32) {
        (self.locale, self.platform, self.block_table_index)
    }

    fn from_reader(rdr: &mut Read) -> io::Result<(HashTableKey, HashTableValue)> {
        let hash_a = try!(rdr.read_u32::<LittleEndian>());
        let hash_b = try!(rdr.read_u32::<LittleEndian>());

        Ok(((hash_a, hash_b), HashTableValue {
            locale: try!(rdr.read_u16::<LittleEndian>()),
            platform: try!(rdr.read_u16::<LittleEndian>()),
            block_table_index: try!(rdr.read_u32::<LittleEndian>()),
        }))
    }
}

#[derive(Debug)]
struct BlockTableEntry {
    offset: u32,
    archived_size: u32,
    size: u32,
    flags: u32,
}

impl BlockTableEntry {
    pub fn to_tuple(&self) -> (u32, u32, u32, u32) {
        (self.offset, self.archived_size, self.size, self.flags)
    }

    fn from_reader(rdr: &mut Read) -> io::Result<BlockTableEntry> {
        Ok(BlockTableEntry {
            offset: try!(rdr.read_u32::<LittleEndian>()),
            archived_size: try!(rdr.read_u32::<LittleEndian>()),
            size: try!(rdr.read_u32::<LittleEndian>()),
            flags: try!(rdr.read_u32::<LittleEndian>()),
        })
    }
}

fn decrypt(table: [Wrapping<u32>; 1280], key: u32, buf: &mut [u8]) {
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

fn decompress(input: &[u8], output: &mut Vec<u8>) -> Result<usize, ()> {
    if input.len() == 0 {
        return Err(());
    }
    match input[0] {
        0 => no_decompress(&input[1..], output),
        2 => zlib_decompress(&input[1..], output),
        16 => bz2_decompress(&input[1..], output),
        _ => Err(()),
    }
}

fn no_decompress(input: &[u8], output: &mut Vec<u8>) -> Result<usize, ()> {
    output.extend(input.iter().cloned());
    Ok(input.len())
}

fn zlib_decompress(input: &[u8], output: &mut Vec<u8>) -> Result<usize, ()> {
    panic!("unimplemented: zlib");
}

fn bz2_decompress(input: &[u8], output: &mut Vec<u8>) -> Result<usize, ()> {
    let buf = bzip2::decompress(input);
    let length = buf.len();
    output.extend(buf.into_iter());
    Ok(length)
}

enum StringHashType {
    TableOffset = 0,
    HashA = 1,
    HashB = 2,
    Table = 3,
}

fn string_hash(table: [Wrapping<u32>; 1280], string: &[u8], hash_type: StringHashType) -> u32 {
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

fn string_hash_normalize(ch: u8) -> Wrapping<u32> {
    if b'a' <= ch && ch <= b'z' {
        Wrapping((ch - b'a' + b'A') as u32)
    } else {
        Wrapping(ch as u32)
    }
}

fn read_hash_table<R: Read+Seek>(
    reader: &mut R,
    header: &FileHeader,
    header_offset: u32,
) -> io::Result<HashMap<HashTableKey, HashTableValue>> {
    let table_offset = header.hash_table_offset + header_offset;
    if table_offset != 205652 { panic!("bad offset {}", table_offset) }
    let table_entries = header.hash_table_entries;

    let mut buffer = vec![0; 16 * table_entries as usize];
    try!(reader.seek(SeekFrom::Start(table_offset as u64)));
    try!(read_exact(reader, &mut buffer));
    let key = string_hash(ENCRYPTION_TABLE, b"(hash table)", StringHashType::Table);
    decrypt(ENCRYPTION_TABLE, key, &mut buffer);

    let mut entry_rdr = Cursor::new(&buffer[..]);
    let mut out = HashMap::new();
    for _ in 0..table_entries {
        let (key, val) = try!(HashTableValue::from_reader(&mut entry_rdr));
        out.insert(key, val);
    }
    Ok(out)
}

fn read_block_table<R: Read+Seek>(
    reader: &mut R,
    header: &FileHeader,
    header_offset: u32,
) -> io::Result<Vec<BlockTableEntry>> {
    let table_offset = header.block_table_offset + header_offset;
    if table_offset != 205908 { panic!("bad offset {}", table_offset) }
    let table_entries = header.block_table_entries;

    let mut buffer = vec![0; 16 * table_entries as usize];
    try!(reader.seek(SeekFrom::Start(table_offset as u64)));
    try!(read_exact(reader, &mut buffer));
    let key = string_hash(ENCRYPTION_TABLE, b"(block table)", StringHashType::Table);
    decrypt(ENCRYPTION_TABLE, key, &mut buffer);

    let mut entry_rdr = Cursor::new(&buffer[..]);
    let mut out = Vec::with_capacity(table_entries as usize);
    for _ in 0..table_entries {
        out.push(try!(BlockTableEntry::from_reader(&mut entry_rdr)));
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::{Archive, BlockTableEntry, HashTableValue};

    static REPLAY_DETAILS: &'static [u8] = include_bytes!("../../testdata/base_build_15405/replay.details");
    static SC2_REPLAY: &'static [u8] = include_bytes!("../../testdata/test.SC2Replay");

    #[test]
    fn test_header_reader() {
        let archive = Archive::load(Cursor::new(SC2_REPLAY)).unwrap();

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
        let mut archive = Archive::load(Cursor::new(SC2_REPLAY)).ok().expect("load fail");
        let entries = archive.block_table;

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
        let mut archive = Archive::load(Cursor::new(SC2_REPLAY)).ok().expect("load fail");
        let entries = archive.hash_table;

        assert_eq!(entries[&(0xD38437CB, 0x07DFEAEC)].to_tuple(), (0x0000, 0x0000, 0x00000009));
        assert_eq!(entries[&(0xAAC2A54B, 0xF4762B95)].to_tuple(), (0x0000, 0x0000, 0x00000002));
        assert_eq!(entries[&(0xC9E5B770, 0x3B18F6B6)].to_tuple(), (0x0000, 0x0000, 0x00000005));
        assert_eq!(entries[&(0x343C087B, 0x278E3682)].to_tuple(), (0x0000, 0x0000, 0x00000004));
        assert_eq!(entries[&(0x3B2B1EA0, 0xB72EF057)].to_tuple(), (0x0000, 0x0000, 0x00000006));
        assert_eq!(entries[&(0x5A7E8BDC, 0xFF253F5C)].to_tuple(), (0x0000, 0x0000, 0x00000001));
        assert_eq!(entries[&(0xFD657910, 0x4E9B98A7)].to_tuple(), (0x0000, 0x0000, 0x00000008));
        assert_eq!(entries[&(0xD383C29C, 0xEF402E92)].to_tuple(), (0x0000, 0x0000, 0x00000000));
        assert_eq!(entries[&(0xFFFFFFFF, 0xFFFFFFFF)].to_tuple(), (0xFFFF, 0xFFFF, 0xFFFFFFFF));
        assert_eq!(entries[&(0x1DA8B0CF, 0xA2CEFF28)].to_tuple(), (0x0000, 0x0000, 0x00000007));
        assert_eq!(entries[&(0x31952289, 0x6A5FFAA3)].to_tuple(), (0x0000, 0x0000, 0x00000003));
    }

    #[test]
    fn test_read_file() {
        let mut archive = Archive::load(Cursor::new(SC2_REPLAY)).ok().expect("load fail");

        let mut buffer = Vec::new();
        let bytes_read = archive.read_file(b"replay.details", &mut buffer).unwrap();
        assert_eq!(bytes_read, REPLAY_DETAILS.len());
        assert_eq!(&buffer[..], REPLAY_DETAILS);
    }
}
