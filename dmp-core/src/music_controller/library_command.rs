use std::sync::Arc;

use parking_lot::RwLock;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

use crate::{
    config::Config,
    music_storage::{
        library::{self, MusicLibrary},
        playlist::{ExternalPlaylist, Playlist, PlaylistFolderItem},
    },
};

use super::{
    controller::{Controller, LibraryCommand, LibraryResponse},
    controller_handle::LibraryCommandInput,
};

impl Controller {
    pub(super) async fn library_loop(
        lib_mail: async_channel::Receiver<LibraryCommandInput>,
        library: &mut MusicLibrary,
        config: Arc<RwLock<Config>>,
    ) -> Result<(), ()> {
        while true {
            let LibraryCommandInput { res_rx, command } = lib_mail.recv().await.unwrap();
            match command {
                LibraryCommand::Song(uuid) => {
                    let (song, i) = library.query_uuid(&uuid).unwrap();
                    res_rx
                        .send(LibraryResponse::Song(song.clone(), i))
                        .await
                        .unwrap();
                }
                LibraryCommand::AllSongs => {
                    res_rx
                        .send(LibraryResponse::AllSongs(library.library.clone()))
                        .await
                        .unwrap();
                }
                LibraryCommand::ExternalPlaylist(uuid) => {
                    let playlist = library.query_playlist_uuid(&uuid).unwrap();
                    res_rx
                        .send(LibraryResponse::ExternalPlaylist(
                            ExternalPlaylist::from_playlist(playlist, library),
                        ))
                        .await
                        .unwrap();
                }
                LibraryCommand::ImportM3UPlayList(path) => {
                    let playlist = Playlist::from_m3u(path, library).unwrap();
                    let uuid = playlist.uuid;
                    let name = playlist.title.clone();
                    library
                        .playlists
                        .items
                        .push(PlaylistFolderItem::List(playlist));

                    res_rx
                        .send(LibraryResponse::ImportM3UPlayList(uuid, name))
                        .await
                        .unwrap();
                }
                LibraryCommand::Save => {
                    library
                        .save(
                            config
                                .read()
                                .libraries
                                .get_library(&library.uuid)
                                .unwrap()
                                .path
                                .clone(),
                        )
                        .unwrap();
                    res_rx.send(LibraryResponse::Ok).await.unwrap();
                }
                LibraryCommand::Playlists => {
                    let mut lists = vec![];
                    library
                        .playlists
                        .lists_recursive()
                        .into_par_iter()
                        .map(|list| (list.uuid, list.title.clone()))
                        .collect_into_vec(&mut lists);

                    res_rx
                        .send(LibraryResponse::Playlists(lists))
                        .await
                        .unwrap();
                }
                LibraryCommand::PlaylistSong {
                    list_uuid,
                    item_uuid,
                } => {
                    let playlist = library.playlists.query_uuid(&list_uuid).unwrap();
                    let Some((uuid, index)) = playlist.query_uuid(&item_uuid) else {
                        todo!()
                    };
                    let Some((song, _)) = library.query_uuid(uuid) else {
                        todo!()
                    };
                    res_rx
                        .send(LibraryResponse::PlaylistSong(song.clone(), index))
                        .await
                        .unwrap();
                }
                LibraryCommand::PlaylistAddSong { playlist, song } => {
                    let playlist = library.query_playlist_uuid_mut(&playlist).unwrap();
                    playlist.add_track(song);
                    library.save(config.read().path.clone()).unwrap();
                    res_rx.send(LibraryResponse::Ok).await.unwrap();
                }
                LibraryCommand::DeletePlaylist(uuid) => {
                    _ = library.playlists.delete_uuid(uuid);
                    let lib_uuid = library.uuid;
                    library
                        .save(
                            config
                                .read()
                                .libraries
                                .get_library(&lib_uuid)
                                .unwrap()
                                .path
                                .clone(),
                        )
                        .unwrap();
                    res_rx.send(LibraryResponse::Ok).await.unwrap();
                }
                _ => {
                    todo!()
                }
            }
        }
        Ok(())
    }
}
