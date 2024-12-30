use std::path::PathBuf;

use kushi::{QueueError, QueueItem};
use uuid::Uuid;

use crate::music_storage::{library::Song, playlist::ExternalPlaylist};

use super::{controller::{ControllerHandle, LibraryCommand, LibraryResponse, PlayerCommand, PlayerError, PlayerLocation, PlayerResponse, QueueCommand, QueueResponse}, queue::{QueueAlbum, QueueSong}};

impl ControllerHandle {
    // The Library Section
    pub async fn lib_get_song(&self, uuid: Uuid) -> (Song, usize) {
        self.lib_mail.send(LibraryCommand::Song(uuid)).await.unwrap();
        let LibraryResponse::Song(song, index) = self.lib_mail.recv().await.unwrap() else {
            unreachable!()
        };
        (song, index)
    }

    pub async fn lib_get_all(&self) -> Vec<Song> {
        self.lib_mail.send(LibraryCommand::AllSongs).await.unwrap();
        let LibraryResponse::AllSongs(songs) = self.lib_mail.recv().await.unwrap() else {
            unreachable!("It has been reached")
        };
        songs
    }

    pub async fn lib_save(&self) {
        self.lib_mail.send(LibraryCommand::Save).await.unwrap();
        let LibraryResponse::Ok = self.lib_mail.recv().await.unwrap() else {
            unreachable!()
        };
    }

    // The Playlist Section
    pub async fn playlist_get(&self, uuid: Uuid) -> Result<ExternalPlaylist, ()> {
        self.lib_mail.send(LibraryCommand::ExternalPlaylist(uuid)).await.unwrap();
        let LibraryResponse::ExternalPlaylist(playlist) = self.lib_mail.recv().await.unwrap() else {
            unreachable!()
        };
        Ok(playlist)
    }

    /// Returns a `Vec<(Uuid, String)>` containing the Uuid of the playlist and the name after
    pub async fn playlist_get_all(&self) -> Vec<(Uuid, String)> {
        self.lib_mail.send(LibraryCommand::Playlists).await.unwrap();
        let LibraryResponse::Playlists(lists) = self.lib_mail.recv().await.unwrap() else {
            unreachable!()
        };
        lists
    }

    pub async fn playlist_import_m3u(&self, path: PathBuf) -> Result<(Uuid, String), ()> {
        self.lib_mail.send(LibraryCommand::ImportM3UPlayList(path)).await.unwrap();
        let LibraryResponse::ImportM3UPlayList(uuid, name) = self.lib_mail.recv().await.unwrap() else { unreachable!("It has been reached") };
        Ok((uuid, name))
    }

    // The Queue Section
    pub async fn queue_append(&self, item: QueueItem<QueueSong, QueueAlbum>) -> Result<(), QueueError> {
        self.queue_mail.send(QueueCommand::Append(item, true)).await.unwrap();
        let QueueResponse::Empty(res) = self.queue_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn queue_remove(&self, index: usize) -> Result<QueueItem<QueueSong, QueueAlbum>, QueueError> {
        self.queue_mail.send(QueueCommand::Remove(index)).await.unwrap();
        let QueueResponse::Item(res) = self.queue_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn queue_get_all(&self) -> Vec<QueueItem<QueueSong, QueueAlbum>> {
        self.queue_mail.send(QueueCommand::Get).await.unwrap();
        let QueueResponse::GetAll(queue) = self.queue_mail.recv().await.unwrap() else {
            unreachable!()
        };
        queue
    }

    // The Player Section
    pub async fn play_now(&self, uuid: Uuid, location: PlayerLocation) -> Result<Song, QueueError> {
        self.player_mail.send(PlayerCommand::PlayNow(uuid, location)).await.unwrap();
        let PlayerResponse::NowPlaying(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn play(&self) -> Result<(), PlayerError> {
        self.player_mail.send(PlayerCommand::Play).await.unwrap();
        let PlayerResponse::Empty(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn pause(&self) -> Result<(), PlayerError> {
        self.player_mail.send(PlayerCommand::Pause).await.unwrap();
        let PlayerResponse::Empty(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn stop(&self) -> Result<(), PlayerError> {
        self.player_mail.send(PlayerCommand::Stop).await.unwrap();
        let PlayerResponse::Empty(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn seek(&self, time: i64) -> Result<(), PlayerError> {
        self.player_mail.send(PlayerCommand::Seek(time)).await.unwrap();
        let PlayerResponse::Empty(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn set_volume(&self, volume: f32) -> () {
        self.player_mail.send(PlayerCommand::SetVolume(volume)).await.unwrap();
        let PlayerResponse::Empty(Ok(())) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
    }

    pub async fn next(&self) -> Result<Song, QueueError> {
        self.player_mail.send(PlayerCommand::NextSong).await.unwrap();
        let PlayerResponse::NowPlaying(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn prev(&self) -> Result<Song, QueueError> {
        self.player_mail.send(PlayerCommand::PrevSong).await.unwrap();
        let PlayerResponse::NowPlaying(res) = self.player_mail.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }
}
