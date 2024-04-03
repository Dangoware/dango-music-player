use std::{fs::File, io::{Error, Read}, time::Duration};

// use chrono::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::library::{AlbumArt, Song, Tag};

use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment, Playlist as List2};
use nestify::nest;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SortOrder {
    Manual,
    Tag(Tag)
}

nest! {
    #[derive(Debug, Clone, Deserialize, Serialize)]*
    pub struct PlaylistFolder {
        name: String,
        items: Vec<
            pub enum PlaylistFolderItem {
                Folder(PlaylistFolder),
                List(Playlist)
            }
        >
    }
}

impl PlaylistFolder {
    pub fn new() -> Self {
        PlaylistFolder {
            name: String::new(),
            items: Vec::new(),
        }
    }
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Playlist {
    uuid: Uuid,
    title: String,
    cover: Option<AlbumArt>,
    tracks: Vec<Uuid>,
    sort_order: SortOrder,
    play_count: i32,
    play_time: Duration,
}
impl Playlist {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn play_count(&self) -> i32 {
        self.play_count
    }
    pub fn play_time(&self) -> Duration {
        self.play_time
    }
    pub fn set_tracks(&mut self, tracks: Vec<Uuid>) {
        self.tracks = tracks;
    }
    pub fn add_track(&mut self, track: Uuid) -> Result<(), Error> {
        self.tracks.push(track);
        Ok(())
    }
    pub fn remove_track(&mut self, index: i32) -> Result<(), Error> {
        let index = index as usize;
        if (self.tracks.len() - 1) >= index {
            self.tracks.remove(index);
        }
        Ok(())
    }
    // pub fn get_index(&self, song_name: &str) -> Option<usize> {
    //     let mut index = 0;
    //     if self.contains_value(&Tag::Title, song_name) {
    //         for track in &self.tracks {
    //             index += 1;
    //             if song_name == track.tags.get_key_value(&Tag::Title).unwrap().1 {
    //                 dbg!("Index gotted! ", index);
    //                 return Some(index);
    //             }
    //         }
    //     }
    //     None
    // }
    pub fn contains_value(&self, tag: &Tag, value: &str) -> bool {
        &self.tracks.iter().for_each(|track| {

        });
        false
    }
    pub fn to_m3u8(&mut self, tracks: Vec<Song>) {
        let seg = tracks
            .iter()
            .map({
                |track| {

                    MediaSegment {
                        uri: track.primary_uri().unwrap().0.to_string().into(), // TODO: error handle this better
                        duration: track.duration.as_millis() as f32,
                        title: Some(track.tags.get_key_value(&Tag::Title).unwrap().1.into()),
                        ..Default::default()
                    }
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
    pub fn from_m3u8(path: &str) -> Result<Playlist, Error> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();

        let parsed = m3u8_rs::parse_playlist(&bytes);

        let playlist = match parsed {
            Result::Ok((i, playlist)) => playlist,
            Result::Err(e) => panic!("Parsing error: \n{}", e),
        };

        match playlist {
            List2::MasterPlaylist(_) => panic!(),
            List2::MediaPlaylist(pl) => {
                let values = pl.segments.iter().map(|seg| seg.uri.to_owned() ).collect::<Vec<String>>();
            }
        }

        todo!()
    }
    fn title(&self) -> &String {
        &self.title
    }
    fn cover(&self) -> Option<&AlbumArt> {
        match &self.cover {
            Some(e) => Some(e),
            None => None,
        }
    }
    fn tracks(&self) -> Vec<Uuid> {
        self.tracks.to_owned()
    }
}



impl Default for Playlist {
    fn default() -> Self {
        Playlist {
            uuid: Uuid::new_v4(),
            title: String::default(),
            cover: None,
            tracks: Vec::default(),
            sort_order: SortOrder::Manual,
            play_count: 0,
            play_time: Duration::from_secs(0),
        }
    }
}

// #[test]
// fn list_to_m3u8() {
//     let lib = ITunesLibrary::from_file(Path::new(
//         "F:\\Music\\Mp3\\Music Main\\iTunes Music Library.xml",
//     ));
//     let mut a = Playlist::new();
//     let c = lib.to_songs();
//     let mut b = c.iter().map(|song| song.to_owned()).collect::<Vec<Song>>();
//     a.tracks.append(&mut b);
//     a.to_m3u8()
// }
