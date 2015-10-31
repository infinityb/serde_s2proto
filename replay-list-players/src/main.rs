
extern crate mpq;
extern crate serde;
extern crate serde_s2proto;

use std::{env, fs};
use std::io::{self, Read, Write};

use serde::de;

use serde_s2proto::format::protocol15405;
use serde_s2proto::VersionedDeserializer;

mod value;

fn main() {
	let filename = env::args_os().nth(1).unwrap();
	let mut file = fs::File::open(&filename).unwrap();
    let mut archive = mpq::Archive::load(file).unwrap();

    let mut replay_details = Vec::new();
    archive.read_file(b"replay.details", &mut replay_details).unwrap();

    let mut des = VersionedDeserializer::new(
    	&replay_details[..],
    	protocol15405::TYPEINFOS,
    	protocol15405::GAME_DETAILS_TYPEID);

    let val: value::Value = de::Deserialize::deserialize(&mut des).unwrap();

    let title = val.get_path(&["m_title"]).and_then(|x| x.as_str()).unwrap();
    println!("Map Title : {}", title);

    let player_list = val.get_path(&["m_playerList"]).and_then(|x| x.as_array()).unwrap();

    println!("players:");
    for player in player_list.iter() {
    	let team = player.get_path(&["m_teamId"]).and_then(|x| x.as_u64()).unwrap();
    	let name = player.get_path(&["m_name"]).and_then(|x| x.as_str()).unwrap();
    	let race = player.get_path(&["m_race"]).and_then(|x| x.as_str()).unwrap();
    	println!("  Team {}: {} ({})", team, name, race);
    }
}
