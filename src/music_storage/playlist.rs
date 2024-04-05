use std::{fs::File, io::Read, path:: PathBuf, sync::{Arc, RwLock}};
use std::error::Error;

use chrono::Duration;
use uuid::Uuid;
use super::library::{AlbumArt, MusicLibrary, Song, Tag, URI};

use m3u8_rs::{MediaPlaylist, MediaPlaylistType, MediaSegment, Playlist as List2};

#[derive(Debug, Clone)]
pub enum SortOrder {
    Manual,
    Tag(Tag)
}
#[derive(Debug, Clone)]
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
    pub fn play_time(&self) -> chrono::Duration {
        self.play_time
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
    pub fn contains_value(&self, tag: &Tag, value: &String, lib: Arc<RwLock<MusicLibrary>>) -> bool {
        let lib = lib.read().unwrap();
        let items = match lib.query_tracks(value, &vec![tag.to_owned()], &vec![tag.to_owned()]) {
            Some(e) => e,
            None => return false
        };

        for item in items {
            for uuid in &self.tracks {
                if uuid == &item.uuid {
                    return true;
                }
            }
        }

        false
    }

    pub fn to_m3u8(&mut self, lib: Arc<RwLock<MusicLibrary>>, location: &str) -> Result<(), Box<dyn Error>> {
        let lib = lib.read().unwrap();
        let seg = self.tracks
            .iter()
            .filter_map( |uuid| {
                    if let Some((track, _)) = lib.query_uuid(uuid) {
                        if let URI::Local(_) = track.location {
                            Some(MediaSegment {
                                uri: track.location.to_string(),
                                duration: track.duration.as_millis() as f32,
                                title: track.tags.get_key_value(&Tag::Title).map(|tag| tag.1.into()),
                                ..Default::default()
                            })
                        }else { None }
                    }else { None }
                }
            )
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

    pub fn from_m3u8(path: &str, lib: Arc<RwLock<MusicLibrary>>) -> Result<Playlist, Box<dyn Error>> {
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
            List2::MasterPlaylist(_) => Err("This is a Master Playlist!\nPlase input a Media Playlist".into()),
            List2::MediaPlaylist(playlist_) => {
                let mut uuids = Vec::new();
                for seg in playlist_.segments {
                    let path_ = PathBuf::from(seg.uri.to_owned());
                    let mut lib = lib.write().unwrap();

                    let uuid = if let Some((song, _)) = lib.query_uri(&URI::Local(path_.clone())) {
                        song.uuid
                    }else {
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
                    playlist.title = path.split("\\")
                    .last()
                    .unwrap_or_default()
                    .strip_suffix(".m3u8")
                    .unwrap_or_default()
                    .to_string();
                }
                #[cfg(target_family = "unix")]
                {
                    playlist.title = path.split("/")
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
            play_time: Duration::zero(),
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
        let tracks = lib.library.iter().map(|track| track.uuid ).collect();
        playlist.set_tracks(tracks);

        _ = playlist.to_m3u8(Arc::new(RwLock::from(lib)), ".\\test-config\\playlists\\playlist.m3u8");
    }

    #[test]
    fn m3u8_to_list() {
        let (_, lib) = read_config_lib();
        let arc = Arc::new(RwLock::from(lib));
        let playlist = Playlist::from_m3u8(".\\test-config\\playlists\\playlist.m3u8", arc).unwrap();
        dbg!(playlist);
    }
}
