use chrono::{DateTime, Utc};
use std::{fs::File, io::Read, path::PathBuf, time::Duration};

use crate::music_storage::db_reader::common::{get_bytes, get_bytes_vec, get_datetime};

const MAGIC: [u8; 16] = [
    0xE1, 0xA0, 0x9C, 0x91, 0xF8, 0x3C, 0x77, 0x42, 0x85, 0x2C, 0x3B, 0xCC, 0x14, 0x01, 0xD3, 0xF2,
];

#[derive(Debug)]
pub struct FoobarPlaylist {
    path: PathBuf,
    metadata: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct FoobarPlaylistTrack {
    flags: i32,
    file_name: String,
    subsong_index: i32,
    file_size: i64,
    file_time: DateTime<Utc>,
    duration: Duration,
    rpg_album: u32,
    rpg_track: u32,
    rpk_album: u32,
    rpk_track: u32,
    entries: Vec<(String, String)>,
}

impl FoobarPlaylist {
    pub fn new(path: &String) -> Self {
        FoobarPlaylist {
            path: PathBuf::from(path),
            metadata: Vec::new(),
        }
    }

    fn get_meta_offset(&self, offset: usize) -> String {
        let mut result_vec = Vec::new();

        let mut i = offset;
        loop {
            if self.metadata[i] == 0x00 {
                break;
            }

            result_vec.push(self.metadata[i]);
            i += 1;
        }

        String::from_utf8_lossy(&result_vec).into()
    }

    /// Reads the entire MusicBee library and returns relevant values
    /// as a `Vec` of `Song`s
    pub fn read(&mut self) -> Result<Vec<FoobarPlaylistTrack>, Box<dyn std::error::Error>> {
        let mut f = File::open(&self.path).unwrap();
        let mut buffer = Vec::new();
        let mut retrieved_songs: Vec<FoobarPlaylistTrack> = Vec::new();

        // Read the whole file
        f.read_to_end(&mut buffer)?;

        let mut buf_iter = buffer.into_iter();

        // Parse the header
        let magic = get_bytes::<16>(&mut buf_iter);
        if magic != MAGIC {
            return Err("Magic bytes mismatch!".into());
        }

        let meta_size = i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize;
        self.metadata = get_bytes_vec(&mut buf_iter, meta_size);
        let track_count = i32::from_le_bytes(get_bytes(&mut buf_iter));

        // Read all the track fields
        for _ in 0..track_count {
            let flags = i32::from_le_bytes(get_bytes(&mut buf_iter));

            let has_metadata = (0x01 & flags) != 0;
            let has_padding = (0x04 & flags) != 0;

            let file_name_offset = i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize;
            let file_name = self.get_meta_offset(file_name_offset);

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

            let file_time = get_datetime(&mut buf_iter, false);

            let duration = Duration::from_nanos(u64::from_le_bytes(get_bytes(&mut buf_iter)) / 100);

            let rpg_album = u32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpg_track = u32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpk_album = u32::from_le_bytes(get_bytes(&mut buf_iter));

            let rpk_track = u32::from_le_bytes(get_bytes(&mut buf_iter));

            get_bytes::<4>(&mut buf_iter);

            let mut entries = Vec::new();
            let primary_count = i32::from_le_bytes(get_bytes(&mut buf_iter));
            let secondary_count = i32::from_le_bytes(get_bytes(&mut buf_iter));
            let _secondary_offset = i32::from_le_bytes(get_bytes(&mut buf_iter));

            // Get primary keys
            for _ in 0..primary_count {
                println!("{}", i32::from_le_bytes(get_bytes(&mut buf_iter)));

                let key =
                    self.get_meta_offset(i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize);

                entries.push((key, String::new()));
            }

            // Consume unknown 32 bit value
            println!("unk");
            get_bytes::<4>(&mut buf_iter);

            // Get primary values
            for i in 0..primary_count {
                println!("primkey {i}");

                let value =
                    self.get_meta_offset(i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize);

                entries[i as usize].1 = value;
            }

            // Get secondary Keys
            for _ in 0..secondary_count {
                let key =
                    self.get_meta_offset(i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize);
                let value =
                    self.get_meta_offset(i32::from_le_bytes(get_bytes(&mut buf_iter)) as usize);
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

        Ok(retrieved_songs)
    }
}
