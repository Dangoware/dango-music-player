use quick_xml::events::Event;
use quick_xml::reader::Reader;

use std::collections::{BTreeMap, HashMap};
use std::io::Error;
use std::path::PathBuf;
use std::str::FromStr;
use std::vec::Vec;

use chrono::prelude::*;

use crate::music_storage::db_reader::extern_library::ExternalLibrary;

#[derive(Debug, Default, Clone)]
pub struct XmlLibrary {
    tracks: Vec<XMLSong>
}
impl XmlLibrary {
    fn new() -> Self {
        Default::default()
    }
}
impl ExternalLibrary for XmlLibrary {
    fn from_file(&mut self, file: &PathBuf) -> Self {
        let mut reader = Reader::from_file(file).unwrap();
        reader.trim_text(true);
        //count every event, for fun ig?
        let mut count = 0;
        //count for skipping useless beginning key
        let mut count2 = 0;
        //number of grabbed songs
        let mut count3 = 0;
        //number of IDs skipped
        let mut count4 = 0;

        let mut buf = Vec::new();
        let mut skip = false;

        let mut converted_songs: Vec<XMLSong> = Vec::new();


        let mut song_tags: HashMap<String, String> = HashMap::new();
        let mut key: String = String::new();
        let mut tagvalue: String = String::new();
        let mut key_selected = false;

        use std::time::Instant;
        let now = Instant::now();

        loop {
            //push tag to song_tags map
            if !key.is_empty() && !tagvalue.is_empty() {
                song_tags.insert(key.clone(), tagvalue.clone());
                key.clear();
                tagvalue.clear();
                key_selected = false;

                //end the song to start a new one, and turn turn current song map into XMLSong
                if song_tags.contains_key(&"Location".to_string()) {
                    count3 += 1;
                    //check for skipped IDs
                    if &count3.to_string()
                        != song_tags.get_key_value(&"Track ID".to_string()).unwrap().1
                    {
                        count3 += 1;
                        count4 += 1;
                    }
                    converted_songs.push(XMLSong::from_hashmap(&mut song_tags).unwrap());
                    song_tags.clear();
                    skip = true;
                }
            }
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(_)) => {
                    count += 1;
                    count2 += 1;
                }
                Ok(Event::Text(e)) => {
                    if count < 17 && count != 10 {
                        continue;
                    }else if skip {
                        skip = false;
                        continue;
                    }

                    let text = e.unescape().unwrap().to_string();

                    if text == count2.to_string() && !key_selected {
                        continue;
                    }

                    //Add the key/value depenidng on if the key is selected or not ⛩️sorry buzz

                    match key_selected {
                        true => tagvalue.push_str(&text),
                        false => {
                            key.push_str(&text);
                            if !key.is_empty() {
                                key_selected = true
                            } else {
                                panic!("Key not selected?!")
                            }
                        }
                        _ => panic!("WHAT DID YOU JUST DO?!🐰🐰🐰🐰"),
                    }
                }
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        let elasped = now.elapsed();
        println!("\n\nXMLReader\n=========================================\n\nDone!\n{} songs grabbed in {:#?}\nIDs Skipped: {}", count3, elasped, count4);
        // dbg!(folder);
        self.tracks.append(converted_songs.as_mut());
        self.clone()
    }
}
#[derive(Debug, Clone, Default)]
pub struct XMLSong {
    pub id: i32,
    pub plays: i32,
    pub favorited: bool,
    pub banned: bool,
    pub rating: Option<u8>,
    pub format: Option<String>,
    pub song_type: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    pub date_added: Option<DateTime<Utc>>,
    pub date_modified: Option<DateTime<Utc>>,
    pub tags: BTreeMap<String, String>,
    pub location: String,
}

impl XMLSong {
    pub fn new() -> XMLSong {
        Default::default()
    }


    fn from_hashmap(map: &mut HashMap<String, String>) -> Result<XMLSong, Error> {
        let mut song = XMLSong::new();
        //get the path with the first bit chopped off
        let path_: String = map.get_key_value("Location").unwrap().1.clone();
        let track_type: String = map.get_key_value("Track Type").unwrap().1.clone();
        let path: String = match track_type.as_str() {
            "File" => {
                if path_.contains("file://localhost/") {
                    path_.strip_prefix("file://localhost/").unwrap();
                }
                path_
            }
            "URL" => path_,
            _ => path_,
        };

        for (key, value) in map {
            match key.as_str() {
                "Track ID" => song.id = value.parse().unwrap(),
                "Location" => song.location = path.to_string(),
                "Play Count" => song.plays = value.parse().unwrap(),
                "Love" => {
                    //check if the track is (L)Loved or (B)Banned
                    match value.as_str() {
                        "L" => song.favorited = true,
                        "B" => song.banned = false,
                        _ => continue,
                    }
                }
                "Rating" => song.rating = Some(value.parse().unwrap()),
                "Kind" => song.format = Some(value.to_string()),
                "Play Date UTC" => {
                    song.last_played = Some(DateTime::<Utc>::from_str(value).unwrap())
                }
                "Date Added" => song.date_added = Some(DateTime::<Utc>::from_str(value).unwrap()),
                "Date Modified" => {
                    song.date_modified = Some(DateTime::<Utc>::from_str(value).unwrap())
                }
                "Track Type" => song.song_type = Some(value.to_string()),
                _ => {
                    song.tags.insert(key.to_string(), value.to_string());
                }
            }
        }
        // println!("{:.2?}", song);
        Ok(song)
    }
}


pub fn get_folder(file: &PathBuf) -> String {
    let mut reader = Reader::from_file(file).unwrap();
    reader.trim_text(true);
    //count every event, for fun ig?
    let mut count = 0;
    let mut buf = Vec::new();
    let mut folder = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => {
                count += 1;
            }
            Ok(Event::Text(e)) => {
                if count == 10 {
                    folder = String::from(
                        e.unescape()
                            .unwrap()
                            .to_string()
                            .strip_prefix("file://localhost/")
                            .unwrap(),
                    );
                    return folder;
                }
            }
            Err(_e) => {
                panic!("oh no! something happened in the public function `get_reader_from_xml()!`")
            }
            _ => (),
        }
    }
}
