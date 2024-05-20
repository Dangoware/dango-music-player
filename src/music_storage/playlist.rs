use std::error::Error;
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use std::time::Duration;

// use chrono::Duration;
use super::library::{AlbumArt, MusicLibrary, Song, Tag, URI};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment, Playlist as List2};
use nestify::nest;

use rayon::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Manual,
    Tag(Vec<Tag>),
}

nest! {
    #[derive(Debug, Clone, Deserialize, Serialize)]*
    #[derive(Default)]
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

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn cover(&self) -> Option<&AlbumArt> {
        match &self.cover {
            Some(e) => Some(e),
            None => None,
        }
    }

    pub fn tracks(&self) -> Vec<Uuid> {
        self.tracks.to_owned()
    }

    pub fn set_tracks(&mut self, tracks: Vec<Uuid>) {
        self.tracks = tracks;
    }

    pub fn add_track(&mut self, track: Uuid) {
        self.tracks.push(track);
    }

    pub fn remove_track(&mut self, index: i32) {
        let index = index as usize;
        if (self.tracks.len() - 1) >= index {
            self.tracks.remove(index);
        }
    }
    pub fn get_index(&self, uuid: Uuid) -> Option<usize> {
        let mut i = 0;
        if self.contains(uuid) {
            for track in &self.tracks {
                i += 1;
                if &uuid == track {
                    dbg!("Index gotted! ", i);
                    return Some(i);
                }
            }
        }
        None
    }
    pub fn contains(&self, uuid: Uuid) -> bool {
        self.get_index(uuid).is_some()
    }

    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn Error>> {
        super::utils::write_file(self, PathBuf::from(path))?;
        Ok(())
    }

    pub fn from_file(path: &str) -> Result<Playlist, Box<dyn Error>> {
        super::utils::read_file(PathBuf::from(path))
    }

    pub fn to_m3u8(
        &mut self,
        lib: Arc<RwLock<MusicLibrary>>,
        location: &str,
    ) -> Result<(), Box<dyn Error>> {
        let lib = lib.read().unwrap();
        let seg = self
            .tracks
            .iter()
            .filter_map(|uuid| {
                // TODO: The Unwraps need to be handled here
                if let Some((track, _)) = lib.query_uuid(uuid) {
                    if let URI::Local(_) = track.primary_uri().unwrap().0 {
                        Some(MediaSegment {
                            uri: track.primary_uri().unwrap().0.to_string(),
                            duration: track.duration.as_millis() as f32,
                            title: track
                                .tags
                                .get_key_value(&Tag::Title)
                                .map(|tag| tag.1.into()),
                            ..Default::default()
                        })
                    } else {
                        None
                    }
                } else {
                    None
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

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .create(true)
            .truncate(true)
            .write(true)
            .open(location)?;
        m3u8.write_to(&mut file)?;
        Ok(())
    }

    pub fn from_m3u8(
        path: &str,
        lib: Arc<RwLock<MusicLibrary>>,
    ) -> Result<Playlist, Box<dyn Error>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();

        let parsed = m3u8_rs::parse_playlist(&bytes);

        let playlist = match parsed {
            Result::Ok((_, playlist)) => playlist,
            Result::Err(e) => panic!("Parsing error: \n{}", e),
        };

        match playlist {
            List2::MasterPlaylist(_) => {
                Err("This is a Master Playlist!\nPlase input a Media Playlist".into())
            }
            List2::MediaPlaylist(playlist_) => {
                let mut uuids = Vec::new();
                for seg in playlist_.segments {
                    let path_ = PathBuf::from(seg.uri.to_owned());
                    let mut lib = lib.write().unwrap();

                    let uuid = if let Some((song, _)) = lib.query_uri(&URI::Local(path_.clone())) {
                        song.uuid
                    } else {
                        let song_ = Song::from_file(&path_)?;
                        let uuid = song_.uuid.to_owned();
                        lib.add_song(song_)?;
                        uuid
                    };
                    uuids.push(uuid);
                }
                let mut playlist = Playlist::new();

                #[cfg(target_family = "windows")]
                {
                    playlist.title = path
                        .split('\\')
                        .last()
                        .unwrap_or_default()
                        .strip_suffix(".m3u8")
                        .unwrap_or_default()
                        .to_string();
                }
                #[cfg(target_family = "unix")]
                {
                    playlist.title = path
                        .split("/")
                        .last()
                        .unwrap_or_default()
                        .strip_suffix(".m3u8")
                        .unwrap_or_default()
                        .to_string();
                }

                playlist.set_tracks(uuids);
                Ok(playlist)
            }
        }
    }

    pub fn out_tracks(&self, lib: Arc<RwLock<MusicLibrary>>) -> (Vec<Song>, Vec<&Uuid>) {
        let lib = lib.read().unwrap();
        let mut songs = vec![];
        let mut invalid_uuids = vec![];

        for uuid in &self.tracks {
            if let Some((track, _)) = lib.query_uuid(uuid) {
                songs.push(track.to_owned());
            } else {
                invalid_uuids.push(uuid);
            }
        }

        if let SortOrder::Tag(sort_by) = &self.sort_order {
            println!("sorting by: {:?}", sort_by);

            songs.par_sort_by(|a, b| {
                for (i, sort_option) in sort_by.iter().enumerate() {
                    dbg!(&i);
                    let tag_a = match sort_option {
                        Tag::Field(field_selection) => {
                            match a.get_field(field_selection.as_str()) {
                                Some(field_value) => field_value.to_string(),
                                None => continue,
                            }
                        }
                        _ => match a.get_tag(sort_option) {
                            Some(tag_value) => tag_value.to_owned(),
                            None => continue,
                        },
                    };

                    let tag_b = match sort_option {
                        Tag::Field(field_selection) => match b.get_field(field_selection) {
                            Some(field_value) => field_value.to_string(),
                            None => continue,
                        },
                        _ => match b.get_tag(sort_option) {
                            Some(tag_value) => tag_value.to_owned(),
                            None => continue,
                        },
                    };
                    dbg!(&i);

                    if let (Ok(num_a), Ok(num_b)) = (tag_a.parse::<i32>(), tag_b.parse::<i32>()) {
                        // If parsing succeeds, compare as numbers
                        return dbg!(num_a.cmp(&num_b));
                    } else {
                        // If parsing fails, compare as strings
                        return dbg!(tag_a.cmp(&tag_b));
                    }
                }

                // If all tags are equal, sort by Track number
                let path_a = PathBuf::from(a.get_field("location").unwrap().to_string());
                let path_b = PathBuf::from(b.get_field("location").unwrap().to_string());

                path_a.file_name().cmp(&path_b.file_name())
            })
        }

        (songs, invalid_uuids)
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

#[cfg(test)]
mod test_super {
    use super::*;
    use crate::config::config::tests::read_config_lib;

    #[test]
    fn list_to_m3u8() {
        let (_, lib) = read_config_lib();
        let mut playlist = Playlist::new();
        let tracks = lib.library.iter().map(|track| track.uuid).collect();
        playlist.set_tracks(tracks);

        _ = playlist.to_m3u8(
            Arc::new(RwLock::from(lib)),
            ".\\test-config\\playlists\\playlist.m3u8",
        );
    }

    fn m3u8_to_list() -> Playlist {
        let (_, lib) = read_config_lib();
        let arc = Arc::new(RwLock::from(lib));
        let playlist =
            Playlist::from_m3u8(".\\test-config\\playlists\\playlist.m3u8", arc).unwrap();

        _ = playlist.to_file(".\\test-config\\playlists\\playlist");
        dbg!(playlist)
    }

    #[test]
    fn out_queue_sort() {
        let (_, lib) = read_config_lib();
        let mut list = m3u8_to_list();
        list.sort_order = SortOrder::Tag(vec![Tag::Album]);

        let songs = &list.out_tracks(Arc::new(RwLock::from(lib)));

        dbg!(songs);
    }
}
