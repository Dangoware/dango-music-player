use std::path::Path;

use chrono::Duration;
use walkdir::Error;

use super::{
    library::{AlbumArt, Song, Tag},
    music_collection::MusicCollection, db_reader::{
        xml::reader::XmlLibrary,
        extern_library::ExternalLibrary
    },
};

use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment};
// use nom::IResult;

#[derive(Debug, Clone)]
pub struct Playlist<'a> {
    title: String,
    cover: Option<&'a AlbumArt>,
    tracks: Vec<Song>,
    play_count: i32,
    play_time: Duration,
}
impl<'a> Playlist<'a> {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn play_count(&self) -> i32 {
        self.play_count
    }
    pub fn play_time(&self) -> chrono::Duration {
        self.play_time
    }
    pub fn set_tracks(&mut self, songs: Vec<Song>) -> Result<(), Error> {
        self.tracks = songs;
        Ok(())
    }
    pub fn add_track(&mut self, song: Song) -> Result<(), Error> {
        self.tracks.push(song);
        Ok(())
    }
    pub fn remove_track(&mut self, index: i32) -> Result<(), Error> {
        let bun: usize = index as usize;
        let mut name = String::new();
        if self.tracks.len() >= bun {
            name = String::from(self.tracks[bun].tags.get_key_value(&Tag::Title).unwrap().1);
            self.tracks.remove(bun);
        }
        dbg!(name);
        Ok(())
    }
    pub fn get_index(&self, song_name: &str) -> Option<usize> {
        let mut index = 0;
        if self.contains_value(&Tag::Title, song_name) {
            for track in &self.tracks {
                index += 1;
                if song_name == track.tags.get_key_value(&Tag::Title).unwrap().1 {
                    dbg!("Index gotted! ", index);
                    return Some(index);
                }
            }
        }
        None
    }
    pub fn contains_value(&self, tag: &Tag, value: &str) -> bool {
        for track in &self.tracks {
            if value == track.tags.get_key_value(tag).unwrap().1 {
                return true;
            }
        }
        false
    }
    pub fn to_m3u8(&mut self) {
        let seg = &self
            .tracks
            .iter()
            .map({
                |track| MediaSegment {
                    uri: track.location.to_string().into(),
                    duration: track.duration.as_millis() as f32,
                    title: Some(track.tags.get_key_value(&Tag::Title).unwrap().1.into()),
                    ..Default::default()
                }
            })
            .collect::<Vec<MediaSegment>>();

        let m3u8 = MediaPlaylist {
            version: Some(6),
            target_duration: 3.0,
            media_sequence: 338559,
            discontinuity_sequence: 1234,
            end_list: true,
            playlist_type: Some(MediaPlaylistType::Vod),
            segments: seg.clone(),
            ..Default::default()
        };
        //TODO: change this to put in a real file path
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .create(true)
            .write(true)
            .open("F:\\Dango Music Player\\playlist.m3u8")
            .unwrap();
        m3u8.write_to(&mut file).unwrap();
    }
    pub fn from_file(file: std::fs::File) -> Playlist<'a> {
        todo!()
    }
}
impl MusicCollection for Playlist<'_> {
    fn title(&self) -> &String {
        &self.title
    }
    fn cover(&self) -> Option<&AlbumArt> {
        match self.cover {
            Some(e) => Some(e),
            None => None,
        }
    }
    fn tracks(&self) -> Vec<Song> {
        self.tracks
    }
}
impl Default for Playlist<'_> {
    fn default() -> Self {
        Playlist {
            title: String::default(),
            cover: None,
            tracks: Vec::default(),
            play_count: 0,
            play_time: Duration::zero(),
        }
    }
}

#[test]
fn list_to_m3u8() {
    let lib = XmlLibrary::from_file(Path::new(
        "F:\\Music\\Mp3\\Music Main\\iTunes Music Library.xml",
    ));
    let mut a = Playlist::new();
    let c = lib.to_songs();
    let mut b = c.iter().map(|song| song).collect::<Vec<&Song>>();
    a.tracks.append(&mut b);
    a.to_m3u8()
}
