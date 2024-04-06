use std::{
    sync::{Arc, RwLock},
    error::Error,
};

use listenbrainz::ListenBrainz;
use uuid::Uuid;

use crate::{
    config::config::Config, music_controller::controller::{Controller, QueueCmd, QueueResponse}, music_storage::library::{MusicLibrary, Song, Tag}
};

use super::controller::DatabaseResponse;



impl Controller {
    pub fn listenbrainz_authenticate(&mut self) -> Result<ListenBrainz, Box<dyn Error>> {
        let config = &self.config.read().unwrap();
        let mut client = ListenBrainz::new();

        let lbz_token = match &config.connections.listenbrainz_token {
            Some(token) => token,
            None => todo!("No ListenBrainz token in config")
        };

        if !client.is_authenticated() {
            client.authenticate(lbz_token)?;
        }

        Ok(client)
    }
    pub fn lbz_scrobble(&self, client: ListenBrainz, uuid: Uuid) -> Result<(), Box<dyn Error>> {
        let config = &self.config.read().unwrap();

        &self.db_mail.send(super::controller::DatabaseCmd::QueryUuid(uuid));
        let res = &self.db_mail.recv()?;
        let song = match res {
                DatabaseResponse::Song(song) => song,
                _ => todo!()
        };
        let unknown = &"unknown".to_string();
        let artist = song.get_tag(&Tag::Artist).unwrap_or(unknown);
        let track = song.get_tag(&Tag::Title).unwrap_or(unknown);
        let release = song.get_tag(&Tag::Album).map(|rel| rel.as_str());

        client.listen(artist, track, release)?;
        Ok(())
    }

    pub fn lbz_now_playing(&self, client: ListenBrainz, uuid: Uuid) -> Result<(), Box<dyn Error>> {
        let config = &self.config.read().unwrap();

        &self.db_mail.send(super::controller::DatabaseCmd::QueryUuid(uuid));
        let res = &self.db_mail.recv()?;
        let song = match res {
                DatabaseResponse::Song(song) => song,
                _ => todo!()
        };
        let unknown = &"unknown".to_string();
        let artist = song.get_tag(&Tag::Artist).unwrap_or(unknown);
        let track = song.get_tag(&Tag::Title).unwrap_or(unknown);
        let release = song.get_tag(&Tag::Album).map(|rel| rel.as_str());

        client.listen(artist, track, release)?;
        Ok(())
    }
}

#[cfg(test)]
mod test_super {
    use std::{thread::sleep, time::Duration};

    use super::*;
    use crate::config::config::tests::read_config_lib;

    #[test]
    fn listenbrainz() {
        let mut c = Controller::start(".\\test-config\\config_test.json").unwrap();

        let client = c.listenbrainz_authenticate().unwrap();

        c.q_new().unwrap();
        c.queue_mail[0].send(QueueCmd::SetVolume(0.04)).unwrap();

        let songs = c.lib_get_songs();

        c.q_enqueue(0, songs[1].location.to_owned()).unwrap();
        c.q_play(0).unwrap();


        sleep(Duration::from_secs(100));
        c.lbz_scrobble(client, songs[1].uuid).unwrap();
    }
}
