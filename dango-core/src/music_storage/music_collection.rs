use crate::music_storage::library::{ AlbumArt, Song };

pub trait MusicCollection {
    fn title(&self) -> &String;
    fn cover(&self) -> Option<&AlbumArt>;
    fn tracks(&self) -> Vec<&Song>;
}

