use file_format::FileFormat;
use lofty::{AudioFile, LoftyError, ParseOptions, Probe, TagType, TaggedFileExt};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use uuid::Uuid;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration as StdDur;
use std::vec::Vec;

use chrono::prelude::*;

use crate::music_storage::db_reader::extern_library::ExternalLibrary;
use crate::music_storage::library::{AlbumArt, BannedType, Service, Song, Tag, URI};
use crate::music_storage::utils;

use urlencoding::decode;

#[derive(Debug, Default, Clone)]
pub struct ITunesLibrary {
    tracks: Vec<ITunesSong>,
}
impl ITunesLibrary {
    fn new() -> Self {
        Default::default()
    }
    pub fn tracks(self) -> Vec<ITunesSong> {
        self.tracks
    }
}
impl ExternalLibrary for ITunesLibrary {
    fn from_file(file: &Path) -> Self {
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

        let mut converted_songs: Vec<ITunesSong> = Vec::new();

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

                //end the song to start a new one, and turn turn current song map into iTunesSong
                if song_tags.contains_key(&"Location".to_string()) {
                    count3 += 1;
                    //check for skipped IDs
                    if &count3.to_string()
                        != song_tags.get_key_value(&"Track ID".to_string()).unwrap().1
                    {
                        count3 += 1;
                        count4 += 1;
                    }
                    converted_songs.push(ITunesSong::from_hashmap(&mut song_tags).unwrap());
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
                    } else if skip {
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
                    }
                }
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                Ok(Event::Eof) => break,
                _ => (),
            }
            buf.clear();
        }
        let elasped = now.elapsed();
        println!("\n\niTunesReader grabbed {} songs in {:#?} seconds\nIDs Skipped: {}", count3, elasped.as_secs(), count4);
        let mut lib = ITunesLibrary::new();
        lib.tracks.append(converted_songs.as_mut());
        lib
    }
    fn to_songs(&self) -> Vec<crate::music_storage::library::Song> {
        let mut count = 0;
        let mut bun: Vec<Song> = Vec::new();
        for track in &self.tracks {
            //grab "other" tags
            let mut tags_: BTreeMap<Tag, String> = BTreeMap::new();
            for (key, val) in &track.tags {
                tags_.insert(to_tag(key.clone()), val.clone());
            }
            //make the path readable
            let loc_ = if track.location.contains("file://localhost/") {
                decode(track.location.strip_prefix("file://localhost/").unwrap())
                    .unwrap()
                    .into_owned()
            } else {
                decode(track.location.as_str()).unwrap().into_owned()
            };
            let loc = loc_.as_str();
            if File::open(loc).is_err() && !loc.contains("http") {
                count += 1;
                dbg!(loc);
                continue;
            }

            let location: URI = if track.location.contains("file://localhost/") {
                URI::Local(PathBuf::from(
                    decode(track.location.strip_prefix("file://localhost/").unwrap())
                        .unwrap()
                        .into_owned()
                        .as_str(),
                ))
            } else {
                URI::Remote(Service::None, decode(&track.location).unwrap().into_owned())
            };
            let dur = match get_duration(Path::new(&loc)) {
                Ok(e) => e,
                Err(e) => {
                    dbg!(e);
                    StdDur::from_secs(0)
                }
            };
            let play_time_ = StdDur::from_secs(track.plays as u64 * dur.as_secs());

            let internal_tags = Vec::new(); // TODO: handle internal tags generation

            let ny: Song = Song {
                location: vec![location],
                uuid: Uuid::new_v4(),
                plays: track.plays,
                skips: 0,
                favorited: track.favorited,
                banned: if track.banned {
                        Some(BannedType::All)
                    }else {
                        None
                    },
                rating: track.rating,
                format: match FileFormat::from_file(PathBuf::from(&loc)) {
                    Ok(e) => Some(e),
                    Err(_) => None,
                },
                duration: dur,
                play_time: play_time_,
                last_played: track.last_played,
                date_added: track.date_added,
                date_modified: track.date_modified,
                album_art: match get_art(Path::new(&loc)) {
                    Ok(e) => e,
                    Err(_) => Vec::new(),
                },
                tags: tags_,
                internal_tags,
            };
            // dbg!(&ny.tags);
            bun.push(ny);
        }
        println!("skipped: {}", count);
        bun
    }
}
fn to_tag(string: String) -> Tag {
    match string.to_lowercase().as_str() {
        "name" => Tag::Title,
        "album" => Tag::Album,
        "artist" => Tag::Artist,
        "album artist" => Tag::AlbumArtist,
        "genre" => Tag::Genre,
        "comment" => Tag::Comment,
        "track number" => Tag::Track,
        "disc number" => Tag::Disk,
        _ => Tag::Key(string),
    }
}
fn get_duration(file: &Path) -> Result<StdDur, lofty::LoftyError> {
    let dur = match Probe::open(file)?.read() {
        Ok(tagged_file) => tagged_file.properties().duration(),

        Err(_) => StdDur::from_secs(0),
    };
    Ok(dur)
}
fn get_art(file: &Path) -> Result<Vec<AlbumArt>, LoftyError> {
    let mut album_art: Vec<AlbumArt> = Vec::new();

    let blank_tag = &lofty::Tag::new(TagType::Id3v2);
    let normal_options = ParseOptions::new().parsing_mode(lofty::ParsingMode::Relaxed);
    let tagged_file: lofty::TaggedFile;

    let tag = match Probe::open(file)?.options(normal_options).read() {
        Ok(e) => {
            tagged_file = e;
            match tagged_file.primary_tag() {
                Some(primary_tag) => primary_tag,

                None => match tagged_file.first_tag() {
                    Some(first_tag) => first_tag,
                    None => blank_tag,
                },
            }
        }
        Err(_) => blank_tag,
    };
    let mut img = match utils::find_images(file) {
        Ok(e) => e,
        Err(_) => Vec::new(),
    };
    if !img.is_empty() {
        album_art.append(img.as_mut());
    }

    for (i, _art) in tag.pictures().iter().enumerate() {
        let new_art = AlbumArt::Embedded(i);

        album_art.push(new_art)
    }

    Ok(album_art)
}

#[derive(Debug, Clone, Default)]
pub struct ITunesSong {
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

impl ITunesSong {
    pub fn new() -> ITunesSong {
        Default::default()
    }

    fn from_hashmap(map: &mut HashMap<String, String>) -> Result<ITunesSong, LoftyError> {
        let mut song = ITunesSong::new();
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

#[cfg(test)]
mod tests {
    use std::{path::{Path, PathBuf}, sync::{Arc, RwLock}};

    use crate::{config::{Config, ConfigLibrary}, music_storage::{db_reader::extern_library::ExternalLibrary, library::MusicLibrary}};

    use super::ITunesLibrary;

    #[test]
    fn itunes_lib_test() {
        let mut config = Config::read_file(PathBuf::from("test-config/config_test.json")).unwrap();
        let config_lib = ConfigLibrary::new(PathBuf::from("test-config/library2"), String::from("library2"), None);
        config.libraries.libraries.push(config_lib.clone());

        let songs = ITunesLibrary::from_file(Path::new("test-config\\iTunesLib.xml")).to_songs();

        let mut library = MusicLibrary::init(Arc::new(RwLock::from(config.clone())), config_lib.uuid).unwrap();

        songs.iter().for_each(|song| library.add_song(song.to_owned()).unwrap());

        config.write_file().unwrap();
        library.save(Arc::new(RwLock::from(config))).unwrap();
    }
}
