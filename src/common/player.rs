use super::color::Color;

// 15405 -> 9
struct Blob(Vec<u8>);

// 15405 -> 14
struct FourCC([u8; 4]);

// 15405 -> 17
struct Toon {
	region: u8, // u8
	program_id: FourCC,
	realm: u32, // u32
	name: String,
}

// 15405 -> 20
struct Player {
	name: Blob,
	toon: Toon,
	race: Blob,
	color: Color,
	control: u8, // u8
	team_id: u8, // u4
	handicap: u8, // u7
	observe: u8, // u4
	result: u8, // u4
}

// 15405 -> 32
struct ReplayDetails {
	player_list: Option<Vec<Player>>,
}

struct ReplayDetailsField {
	PlayerList,
	Title,
	Difficulty,
	Thumbnail,
	IsBlizzardMap,
	TimeUtc,
	TimeLocalOffset,
	Description,
	ImageFilePath,
	MapFilePath,
	CacheHandles,
	MiniSave,
	GameSpeed,
	DefaultDifficulty,
}

struct ReplayDetailsVisitor {
	root_typeinfo: usize,
	typeinfos: &'static [TypeInfo],
}

impl serde::de::Visitor for ReplayDetailsVisitor {
    type Value = Color;

    fn visit_map<V>(&mut self, mut visitor: V) -> Result<Self::Value, V::Error>
        where V: serde::de::MapVisitor
    {
    	let typeinfo = &self.typeinfos[self.root_typeinfo];

        let mut player_list: Option<Option<Vec<Player>>> = None;

        loop {
            match try!(visitor.visit_key()) {
                Vint(0) => {
                    a = Some(try!(visitor.visit_value()));
                },
                Some(ColorField::R) => {
                    r = Some(try!(visitor.visit_value()));
                },
                Some(ColorField::G) => {
                    g = Some(try!(visitor.visit_value()));
                },
                Some(ColorField::B) => {
                    b = Some(try!(visitor.visit_value()));
                },
                None => { break; }
            }
        }

        let a = try!(a.ok_or(V::Error::missing_field("m_a")));
        let r = try!(r.ok_or(V::Error::missing_field("m_r")));
        let g = try!(g.ok_or(V::Error::missing_field("m_g")));
        let b = try!(b.ok_or(V::Error::missing_field("m_b")));
        try!(visitor.end());

        Ok(Color { a: a, r: r, g: g, b: b })    }
}