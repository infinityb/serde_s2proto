use std::io::{self};
use phf::Map as PhfMap;

pub mod protocol15405;

// mod protocol15405_def;
// mod protocol16561_def;
// mod protocol16605_def;
// mod protocol16755_def;
// mod protocol16939_def;
// mod protocol17266_def;
// mod protocol17326_def;
// mod protocol18092_def;
// mod protocol18468_def;
// mod protocol18574_def;
// mod protocol19132_def;
// mod protocol19458_def;
// mod protocol19595_def;
// mod protocol19679_def;
// mod protocol21029_def;
// mod protocol21995_def;
// mod protocol22612_def;
// mod protocol23260_def;
// mod protocol24764_def;
// mod protocol24944_def;
// mod protocol26490_def;
// mod protocol27950_def;
// mod protocol28272_def;
// mod protocol28667_def;
// mod protocol32283_def;
// mod protocol34784_def;
// mod protocol34835_def;
// mod protocol36442_def;

pub type TypeId = u32;

pub enum ReplayHeader {}

pub enum ReplayInitData {}

pub enum ReplayDetails {}

pub enum ReplayGameEvents {}

pub enum ReplayMessageEvents {}

pub enum ReplayTrackerEvents {}

pub enum ReplayAttributesEvents {}

pub type ChoiceTypeMap = PhfMap<u32, (&'static str, TypeId)>;

/// name, type, tag
pub type StructField = (&'static str, TypeId, i32);

#[derive(Copy, Clone, Debug)]
pub struct IntBounds {
    pub min: i64,
    pub bitlen: u8,
}

#[derive(Copy, Clone, Debug)]
pub struct Struct {
    pub fields: &'static [StructField],
}

#[derive(Debug)]
pub enum TypeInfo {
    Array {
        bounds: IntBounds,
        typeid: TypeId,
    },
    BitArray { len: IntBounds },
    Blob { len: IntBounds },
    Bool,
    Choice {
        bounds: IntBounds,
        types: ChoiceTypeMap,
    },
    FourCC,
    Int { bounds: IntBounds },
    Null,
    Optional {
        typeid: TypeId,
    },
    Real32,
    Real64,
    Struct(Struct),
}

pub trait Protocol {
    fn protocol_num(&self) -> u32;

    fn decode_replay_header(&self, rdr: &mut io::Read) -> io::Result<ReplayHeader>;

    fn decode_replay_initdata(&self, rdr: &mut io::Read) -> io::Result<ReplayInitData>;

    fn decode_replay_details(&self, rdr: &mut io::Read) -> io::Result<ReplayDetails>;

    fn decode_replay_game_events(&self, rdr: &mut io::Read) -> io::Result<ReplayGameEvents>;

    fn decode_replay_message_events(&self, rdr: &mut io::Read) -> io::Result<ReplayMessageEvents>;

    fn decode_replay_tracker_events(&self, rdr: &mut io::Read) -> io::Result<ReplayTrackerEvents>;

    fn decode_replay_attributes_events(&self, rdr: &mut io::Read) -> io::Result<ReplayAttributesEvents>;
}
