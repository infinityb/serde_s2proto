use super::color::Color;

// 15405 -> 9
struct Blob(Vec<u8>);

// 15405 -> 14
#[derive(Debug)]
struct FourCC([u8; 4]);

// 15405 -> 17
// #[derive(Serialize, Deserialize, Debug)]
#[derive(Debug)]
struct Toon {
    region: u8, // u8
    program_id: FourCC,
    realm: u32, // u32
    name: String,
}

// 15405 -> 20
// #[derive(Serialize, Deserialize, Debug)]
#[derive(Debug)]
struct Player {
    name: String,
    toon: Toon,
    race: String,
    color: Color,
    control: u8, // u8
    team_id: u8, // u4
    handicap: u8, // u7
    observe: u8, // u4
    result: u8, // u4
}

// #[derive(Serialize, Deserialize, Debug)]
#[derive(Debug)]
struct FileContainer {
    #[serde(rename="m_file")]
    file: String,
}

// 15405 -> 32
// #[derive(Serialize, Deserialize, Debug)]
#[derive(Debug)]
struct ReplayDetails {
    #[serde(rename="m_playerList")]
    player_list: Option<Vec<Player>>,

    #[serde(rename="m_title")]
    title: String,

    #[serde(rename="m_difficulty")]
    difficuly: u8,

    #[serde(rename="m_thumbnail")]
    thumbnail: FileContainer,

    #[serde(rename="m_isBlizzardMap")]
    is_blizzard_map: bool,

    #[serde(rename="m_timeUTC")]
    time_utc: i64,

    #[serde(rename="m_timeLocalOffset")]
    time_local_offset: i64,

    #[serde(rename="m_description")]
    description: String,
    
    #[serde(rename="m_imageFilePath")]
    image_file_path: String,
    
    #[serde(rename="m_mapFileName")]
    map_filename: String,

    #[serde(rename="m_cacheHandles")]
    cache_handles: Option<Vec<String>>,

    #[serde(rename="m_miniSave")]
    mini_save: bool,

    #[serde(rename="m_gameSpeed")]
    game_speed: u8,

    #[serde(rename="m_defaultDifficulty")]
    default_difficulty: u8,
}
