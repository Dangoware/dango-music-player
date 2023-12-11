use std::path::Path;

use crate::music_storage::library::Song;

pub trait ExternalLibrary {
    fn from_file(file: &Path) -> Self;
    fn write(&self) {
        unimplemented!();
    }
    fn to_songs(&self) -> Vec<Song>;
}
