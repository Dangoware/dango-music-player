use super::utils::get_string;
use crate::music_storage::db_reader::common::{get_bytes, get_datetime};
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;

pub struct MusicBeeDatabase {
    path: String,
}

impl MusicBeeDatabase {
    pub fn new(path: String) -> MusicBeeDatabase {
        MusicBeeDatabase { path }
    }

    /// Reads the entire MusicBee library and returns relevant values
    /// as a `Vec` of `Song`s
    pub fn read(&self) -> Result<Vec<MusicBeeSong>, Box<dyn std::error::Error>> {
        let mut f = File::open(&self.path).unwrap();
        let mut buffer = Vec::new();
        let mut retrieved_songs: Vec<MusicBeeSong> = Vec::new();

        // Read the whole file
        f.read_to_end(&mut buffer)?;

        let mut buf_iter = buffer.into_iter();

        // Get the song count from the first 4 bytes
        // and then right shift it by 8 for some reason
        let mut database_song_count = i32::from_le_bytes(get_bytes(&mut buf_iter));
        database_song_count >>= 8;

        let mut song_count = 0;
        loop {
            // If the file designation is 1, then the end of the database
            // has been reached
            let file_designation = match buf_iter.next() {
                Some(1) => break,
                Some(value) => value,
                None => break,
            };

            song_count += 1;

            // Get the file status. Unknown what this means
            let status = buf_iter.next().unwrap();

            buf_iter.next(); // Read in a byte to throw it away

            // Get the play count
            let play_count = u16::from_le_bytes(get_bytes(&mut buf_iter));

            // Get the time the song was last played, stored as a signed 64 bit number of microseconds
            let last_played = get_datetime(buf_iter.by_ref(), true);

            // Get the number of times the song was skipped
            let skip_count = u16::from_le_bytes(get_bytes(&mut buf_iter));

            // Get the path to the song
            let path = get_string(buf_iter.by_ref());

            // Get the file size
            let file_size = i32::from_le_bytes(get_bytes(&mut buf_iter));

            // Get the sample rate
            let sample_rate = i32::from_le_bytes(get_bytes(&mut buf_iter));

            // Get the channel count
            let channel_count = buf_iter.next().unwrap();

            // Get the bitrate type (CBR, VBR, etc.)
            let bitrate_type = buf_iter.next().unwrap();

            // Get the actual bitrate
            let bitrate = i16::from_le_bytes(get_bytes(&mut buf_iter));

            // Get the track length in milliseconds
            let track_length =
                Duration::from_millis(i32::from_le_bytes(get_bytes(&mut buf_iter)) as u64);

            // Get the date added and modified in the same format
            let date_added = get_datetime(buf_iter.by_ref(), true);
            let date_modified = get_datetime(buf_iter.by_ref(), true);

            // Gets artwork information
            //
            // Artworks are stored as chunks describing the type
            // (embedded, file), and some other information.
            let mut artwork: Vec<MusicBeeAlbumArt> = vec![];
            loop {
                let artwork_type = buf_iter.next().unwrap();
                if artwork_type > 253 {
                    break;
                }

                let unknown_string = get_string(buf_iter.by_ref());
                let storage_mode = buf_iter.next().unwrap();
                let storage_path = get_string(buf_iter.by_ref());

                artwork.push(MusicBeeAlbumArt {
                    artwork_type,
                    unknown_string,
                    storage_mode,
                    storage_path,
                });
            }

            buf_iter.next(); // Read in a byte to throw it away

            // Gets all the tags on the song in the database
            let mut tags: Vec<MusicBeeTag> = vec![];
            loop {
                // If the tag code is 0, the end of the block has been reached, so break.
                //
                // If the tag code is 255, it pertains to some CUE file values that are not known
                // throw away these values
                let tag_code = match buf_iter.next() {
                    Some(0) => break,
                    Some(255) => {
                        let repeats = u16::from_le_bytes(get_bytes(&mut buf_iter));
                        for _ in 0..(repeats * 13) - 2 {
                            buf_iter.next().unwrap();
                        }

                        255
                    }
                    Some(value) => value,
                    None => panic!(),
                };

                // Get the string value of the tag
                let tag_value = get_string(buf_iter.by_ref());
                tags.push(MusicBeeTag {
                    tag_code,
                    tag_value,
                });
            }

            // Construct the finished song and add it to the vec
            let constructed_song = MusicBeeSong {
                file_designation,
                status,
                play_count,
                last_played,
                skip_count,
                path,
                file_size,
                sample_rate,
                channel_count,
                bitrate_type,
                bitrate,
                track_length,
                date_added,
                date_modified,
                artwork,
                tags,
            };

            retrieved_songs.push(constructed_song);
        }

        println!("The database claims you have: {database_song_count} songs\nThe retrieved number is: {song_count} songs");

        match database_song_count == song_count {
            true => Ok(retrieved_songs),
            false => Err("Song counts do not match!".into()),
        }
    }
}

#[derive(Debug)]
pub struct MusicBeeTag {
    tag_code: u8,
    tag_value: String,
}

#[derive(Debug)]
pub struct MusicBeeAlbumArt {
    artwork_type: u8,
    unknown_string: String,
    storage_mode: u8,
    storage_path: String,
}

#[derive(Debug)]
pub struct MusicBeeSong {
    file_designation: u8,
    status: u8,
    play_count: u16,
    pub last_played: DateTime<Utc>,
    skip_count: u16,
    path: String,
    file_size: i32,
    sample_rate: i32,
    channel_count: u8,
    bitrate_type: u8,
    bitrate: i16,
    track_length: Duration,
    date_added: DateTime<Utc>,
    date_modified: DateTime<Utc>,

    /* Album art stuff */
    artwork: Vec<MusicBeeAlbumArt>,

    /* All tags */
    tags: Vec<MusicBeeTag>,
}

impl MusicBeeSong {
    pub fn get_tag_code(self, code: u8) -> Option<String> {
        for tag in &self.tags {
            if tag.tag_code == code {
                return Some(tag.tag_value.clone());
            }
        }

        None
    }
}
