use std::path::PathBuf;

use dmp_command::command;
use uuid::Uuid;

// #[derive(Debug, PartialEq, PartialOrd, Clone)]
// pub enum LibraryCommand {
//     Song(Uuid),
//     AllSongs,
//     GetLibrary,
//     LibraryRemoveSong(Uuid),
//     ExternalPlaylist(Uuid),
//     PlaylistSong { list_uuid: Uuid, item_uuid: Uuid },
//     Playlist(Uuid),
//     ImportM3UPlayList(PathBuf),
//     Save,
//     Playlists,
//     PlaylistAddSong { playlist: Uuid, song: Uuid },
//     PlaylistRemoveSong { playlist: Uuid, song: Uuid },
//     DeletePlaylist(Uuid),
// }

#[command]
enum TestCommand {
    #[response(PathBuf)]
    #[function(test_function)]
    Test1(Uuid),
    #[response(Uuid)]
    #[function(test_function2)]
    Test2(PathBuf),
}

fn test_function(uuid: Uuid) -> PathBuf {
    todo!()
}
fn test_function2(p: PathBuf) -> Uuid {
    todo!()
}
