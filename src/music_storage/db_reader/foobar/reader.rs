use std::collections::BTreeMap;
use std::{fs::File, io::Read, path::Path, time::Duration};

use uuid::Uuid;

use super::utils::meta_offset;
use crate::music_storage::db_reader::common::{get_bytes, get_bytes_vec};
use crate::music_storage::db_reader::extern_library::ExternalLibrary;
use crate::music_storage::library::{Song, URI};

const MAGIC: [u8; 16] = [
    0xE1, 0xA0, 0x9C, 0x91, 0xF8, 0x3C, 0x77, 0x42, 0x85, 0x2C, 0x3B, 0xCC, 0x14, 0x01, 0xD3, 0xF2,
];

#[derive(Debug)]
pub struct FoobarPlaylist {
    metadata: Vec<u8>,
    songs: Vec<FoobarPlaylistTrack>,
}

impl ExternalLibrary for FoobarPlaylist {
    /// Reads the entire MusicBee library and returns relevant values
    /// as a `Vec` of `Song`s
    fn from_file(file: &Path) -> Self {
        let mut f = File::open(file).unwrap();
        let mut buffer = Vec::new();
        let mut retrieved_songs: Vec<FoobarPlaylistTrack> = Vec::new();

        // Read the whole file
        f.read_to_end(&mut buffer).unwrap();

        let mut buf_iter = buffer.into_iter();

        // Parse the header
        let magic = get_bytes::<16>(&mut buf_iter);
        if magic != MAGIC {
            panic!("Magic bytes mismatch!");
        }

        let meta_size = i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize;
        let metadata = &get_bytes_vec(&mut buf_iter, meta_size);
        let track_count = i32::from_le_bytes(get_bytes(&mut buf_iter));

        // Read all the track fields
        for _ in 0..track_count {
            let flags = i32::from_le_bytes(get_bytes(&mut buf_iter));

            let has_metadata = (0x01 & flags) != 0;
            let has_padding = (0x04 & flags) != 0;

            let file_name_offset = i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize;
            let file_name = meta_offset(metadata, file_name_offset);

            let subsong_index = i32::from_le_bytes(get_bytes(&mut buf_iter));

            if !has_metadata {
                let track = FoobarPlaylistTrack {
                    file_name,
                    subsong_index,
                    ..Default::default()
                };
                retrieved_songs.push(track);
                continue;
            }

            let file_size = i64::from_le_bytes(get_bytes(&mut buf_iter));

            // TODO: Figure out how to make this work properly
            let file_time = i64::from_le_bytes(get_bytes(&mut buf_iter));

            let duration = Duration::from_nanos(u64::from_le_bytes(get_bytes(&mut buf_iter)) / 100);

            let rpg_album = f32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpg_track = f32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpk_album = f32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpk_track = f32::from_le_bytes(get_bytes(&mut buf_iter));

            get_bytes::<4>(&mut buf_iter);

            let mut entries = Vec::new();
            let primary_count = i32::from_le_bytes(get_bytes(&mut buf_iter));
            let secondary_count = i32::from_le_bytes(get_bytes(&mut buf_iter));
            let _secondary_offset = i32::from_le_bytes(get_bytes(&mut buf_iter));

            // Get primary keys
            for _ in 0..primary_count {
                println!("{}", i32::from_le_bytes(get_bytes(&mut buf_iter)));

                let key = meta_offset(
                    metadata,
                    i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize,
                );

                entries.push((key, String::new()));
            }

            // Consume unknown 32 bit value
            println!("unk");
            get_bytes::<4>(&mut buf_iter);

            // Get primary values
            for i in 0..primary_count {
                println!("primkey {i}");

                let value = meta_offset(
                    metadata,
                    i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize,
                );

                entries[i as usize].1 = value;
            }

            // Get secondary Keys
            for _ in 0..secondary_count {
                let key = meta_offset(
                    metadata,
                    i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize,
                );
                let value = meta_offset(
                    metadata,
                    i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize,
                );
                entries.push((key, value));
            }

            if has_padding {
                get_bytes::<64>(&mut buf_iter);
            }

            let track = FoobarPlaylistTrack {
                flags,
                file_name,
                subsong_index,
                file_size,
                file_time,
                duration,
                rpg_album,
                rpg_track,
                rpk_album,
                rpk_track,
                entries,
            };

            retrieved_songs.push(track);
        }

        Self {
            songs: retrieved_songs,
            metadata: metadata.clone(),
        }
    }

    fn to_songs(&self) -> Vec<Song> {
        self.songs.iter().map(|song| song.find_song()).collect()
    }
}

#[derive(Debug, Default)]
pub struct FoobarPlaylistTrack {
    flags: i32,
    file_name: String,
    subsong_index: i32,
    file_size: i64,
    file_time: i64,
    duration: Duration,
    rpg_album: f32,
    rpg_track: f32,
    rpk_album: f32,
    rpk_track: f32,
    entries: Vec<(String, String)>,
}

impl FoobarPlaylistTrack {
    fn find_song(&self) -> Song {
        let location = URI::Local(self.file_name.clone().into());
        let internal_tags = Vec::new();

        Song {
            location: vec![location],
            uuid: Uuid::new_v4(),
            plays: 0,
            skips: 0,
            favorited: false,
            banned: None,
            rating: None,
            format: None,
            duration: self.duration,
            play_time: Duration::from_secs(0),
            last_played: None,
            date_added: None,
            date_modified: None,
            album_art: Vec::new(),
            tags: BTreeMap::new(),
            internal_tags,
        }
    }
}
