use std::path::PathBuf;

use async_channel::{Receiver, Sender};
use kushi::{QueueError, QueueItem};
use uuid::Uuid;

use crate::music_storage::{library::Song, playlist::ExternalPlaylist};

use super::{
    controller::{
        ControllerHandle, LibraryCommand, LibraryResponse, PlayerCommand, PlayerError,
        PlayerLocation, PlayerResponse, QueueCommand, QueueResponse,
    },
    queue::{QueueAlbum, QueueSong},
};

impl ControllerHandle {
    // The Library Section
    pub async fn lib_get_song(&self, uuid: Uuid) -> (Song, usize) {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::Song(uuid));
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::Song(song, index) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        (song, index)
    }

    pub async fn lib_get_all(&self) -> Vec<Song> {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::AllSongs);
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::AllSongs(songs) = tx.recv().await.unwrap() else {
            unreachable!("It has been reached")
        };
        songs
    }

    pub async fn lib_save(&self) {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::Save);
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::Ok = tx.recv().await.unwrap() else {
            unreachable!()
        };
    }

    // The Playlist Section
    pub async fn playlist_get(&self, uuid: Uuid) -> Result<ExternalPlaylist, ()> {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::ExternalPlaylist(uuid));
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::ExternalPlaylist(playlist) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        Ok(playlist)
    }

    /// Returns a `Vec<(Uuid, String)>` containing the Uuid of the playlist and the name after
    pub async fn playlist_get_all(&self) -> Vec<(Uuid, String)> {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::Playlists);
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::Playlists(lists) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        lists
    }

    pub async fn playlist_import_m3u(&self, path: PathBuf) -> Result<(Uuid, String), ()> {
        let (command, tx) = LibraryCommandInput::command(LibraryCommand::ImportM3UPlayList(path));
        self.lib_mail_rx.send(command).await.unwrap();
        let LibraryResponse::ImportM3UPlayList(uuid, name) = tx.recv().await.unwrap() else {
            unreachable!("It has been reached")
        };
        Ok((uuid, name))
    }

    // The Queue Section
    pub async fn queue_append(
        &self,
        item: QueueItem<QueueSong, QueueAlbum>,
    ) -> Result<(), QueueError> {
        let (command, tx) = QueueCommandInput::command(QueueCommand::Append(item, true));
        self.queue_mail_rx.send(command).await.unwrap();
        let QueueResponse::Empty(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn queue_remove(
        &self,
        index: usize,
    ) -> Result<QueueItem<QueueSong, QueueAlbum>, QueueError> {
        let (command, tx) = QueueCommandInput::command(QueueCommand::Remove(index));
        self.queue_mail_rx.send(command).await.unwrap();
        let QueueResponse::Item(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn queue_get_all(&self) -> Vec<QueueItem<QueueSong, QueueAlbum>> {
        let (command, tx) = QueueCommandInput::command(QueueCommand::Get);
        self.queue_mail_rx.send(command).await.unwrap();
        let QueueResponse::GetAll(queue) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        queue
    }

    // The Player Section
    pub async fn play_now(&self, uuid: Uuid, location: PlayerLocation) -> Result<Song, QueueError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::PlayNow(uuid, location));
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::NowPlaying(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn play(&self) -> Result<(), PlayerError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::Play);
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::Empty(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn pause(&self) -> Result<(), PlayerError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::Pause);
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::Empty(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn stop(&self) -> Result<(), PlayerError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::Stop);
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::Empty(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn seek(&self, time: i64) -> Result<(), PlayerError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::Seek(time));
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::Empty(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn set_volume(&self, volume: f32) -> () {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::SetVolume(volume));
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::Empty(Ok(())) = tx.recv().await.unwrap() else {
            unreachable!()
        };
    }

    pub async fn next(&self) -> Result<Song, QueueError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::NextSong);
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::NowPlaying(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    pub async fn prev(&self) -> Result<Song, QueueError> {
        let (command, tx) = PlayerCommandInput::command(PlayerCommand::PrevSong);
        self.player_mail_rx.send(command).await.unwrap();
        let PlayerResponse::NowPlaying(res) = tx.recv().await.unwrap() else {
            unreachable!()
        };
        res
    }

    // The Connections Section
    pub fn discord_rpc(&self, client_id: u64) {
        self.connections_rx.send(super::connections::ConnectionsNotification::TryEnableConnection(super::connections::TryConnectionType::Discord(client_id))).unwrap();
    }

    pub fn listenbrainz_scrobble_auth(&self, token: String) {
        self.connections_rx.send(super::connections::ConnectionsNotification::TryEnableConnection(super::connections::TryConnectionType::ListenBrainz(token))).unwrap();
    }

    pub fn last_fm_scrobble_auth(&self, api_key: String, api_secret: String, auth: super::connections::LastFMAuth) {
        self.connections_rx.send(super::connections::ConnectionsNotification::TryEnableConnection(super::connections::TryConnectionType::LastFM { api_key, api_secret, auth })).unwrap();
    }


}

pub(super) struct LibraryCommandInput {
    pub res_rx: Sender<LibraryResponse>,
    pub command: LibraryCommand,
}

impl LibraryCommandInput {
    pub fn command(command: LibraryCommand) -> (Self, Receiver<LibraryResponse>) {
        let (rx, tx) = async_channel::bounded(1);
        (
            Self {
                res_rx: rx,
                command,
            },
            tx,
        )
    }
}

pub(super) struct QueueCommandInput {
    pub res_rx: Sender<QueueResponse>,
    pub command: QueueCommand,
}

impl QueueCommandInput {
    pub fn command(command: QueueCommand) -> (Self, Receiver<QueueResponse>) {
        let (rx, tx) = async_channel::bounded(1);
        (
            Self {
                res_rx: rx,
                command,
            },
            tx,
        )
    }
}

pub(super) struct PlayerCommandInput {
    pub res_rx: Sender<PlayerResponse>,
    pub command: PlayerCommand,
}

impl PlayerCommandInput {
    pub fn command(command: PlayerCommand) -> (Self, Receiver<PlayerResponse>) {
        let (rx, tx) = async_channel::bounded(1);
        (
            Self {
                res_rx: rx,
                command,
            },
            tx,
        )
    }
}
