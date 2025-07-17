import { MutableRefObject, useEffect } from "react";
import { URI } from "../bindings/URI";
import { PlayerLocation } from "../bindings/PlayerLocation";
import { invoke } from "@tauri-apps/api/core";
import { Menu, Submenu } from "@tauri-apps/api/menu";
import { appWindow, contextMenuPosition } from "./App";
import { Song } from "../bindings/Song";
import { PlaylistInfo } from "./PlaylistHead";

export interface MainViewProps {
  lib_ref: [JSX.Element[], React.Dispatch<React.SetStateAction<JSX.Element[]>>],
  viewName: string,
  playlistsInfo: MutableRefObject<PlaylistInfo[]>,
  setSelected: (props: SongProps) => void,
  selectedSong: MutableRefObject<SongProps | undefined>,
}

export default function MainView({ lib_ref, viewName, playlistsInfo, setSelected, selectedSong }: MainViewProps) {
  const [library, setLibrary] = lib_ref;


  const addToQueue = (_: string) => {
    invoke('add_song_to_queue', { uuid: selectedSong.current?.uuid, location: selectedSong.current?.playerLocation }).then(() => {});
    }
  const playNow = () => invoke("play_now", { uuid: selectedSong.current!.uuid, location: selectedSong.current!.playerLocation }).then(() => {})
  const playNext = () => invoke("play_next_queue", { uuid: selectedSong.current!.uuid, location: selectedSong.current!.playerLocation }).then(() => {})
    const removeLibPlaylist = () => {
    invoke("remove_from_lib_playlist", { song: selectedSong.current!.uuid, location: selectedSong.current!.playerLocation }).then(() => {
      if (selectedSong.current!.reload !== undefined) {
        selectedSong.current!.reload()
        }
      })
    }
    async function clickHandler(event: React.MouseEvent) {
      event.preventDefault();

      await invoke('get_playlists');
      let removeText = "Remove from Library";
      if (selectedSong.current!.playerLocation != "Library") {
        removeText = "Remove from Playlist";
      }
      const menu = await Menu.new({
      items: [
        { id: "play_now_" + selectedSong.current!.uuid, text: "Play Now", action: playNow },
        { id: "play_next_" + selectedSong.current!.uuid, text: "Play Next", action: playNext },
        { id: "add_song_to_queue" + selectedSong.current!.uuid, text: "Add to Queue", action: addToQueue },
          await Submenu.new(
            {
              text: "Add to Playlist...",
            items: [...selectedSong.current!.playlists.current.map((list) => {
                const addToPlaylist = () => {
                invoke('add_song_to_playlist', { playlist: list.uuid, song: selectedSong.current!.uuid }).then(() => {});
                }
              return { id: "add_song_to_playlists" + selectedSong.current!.uuid + list.uuid, text: list.name, action: addToPlaylist }
              })]
            }  
          ),
        { id: "remove_from_lib_playlist" + selectedSong.current!.location + selectedSong.current!.uuid, text: removeText, action: removeLibPlaylist },
      ]});
      menu.popup(await contextMenuPosition(event));
    }


  useEffect(() => {
    const unlisten = appWindow.listen<null>("library_loaded", (_) => {
      console.log("library_loaded");
      invoke('get_library').then((lib) => {
        setLibrary([...(lib as Song[]).map((song, i) => {
          console.log("uri", song.location[0])
          return (
            <MainViewSong
              key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
              location={ song.location }
              playerLocation="Library"
              uuid={ song.uuid }
              plays={ song.plays }
              duration={ song.duration }
              tags={ song.tags }
              playlists={ playlistsInfo }
              index={ i }
              setSelected={ setSelected }
            />
          )
        })])
      })

      invoke('get_playlists').then(() => {})
    })
    return () => { unlisten.then((f) => f()) }
  }, []);


  return (
    <div
      onDoubleClick={ playNow }
      onContextMenu={ clickHandler }
      className="mainView">
      <h1>{ viewName }</h1>
      <div>{ library }</div>
    </div>
  )
}


export interface SongProps {
  location: URI[],
  playerLocation: PlayerLocation,
  uuid: string,
  plays: number,
  format?: string,
  duration: number,
  last_played?: string,
  date_added?: string,
  date_modified?: string,
  tags: any,
  playlists: MutableRefObject<PlaylistInfo[]>,
  index: number,
  setSelected: (props: SongProps) => void,
  reload?: () => void
}

export function MainViewSong(props: SongProps) {
  // console.log(props.tags);
    // useEffect(() => {
  //   const unlistenPromise = listen<string>("add_song_to_queue", (event) => {
  //     switch (event.payload) {
  //       default:
  //         console.log("Unimplemented application menu id:", event.payload);
  //     }
  //   });

  //   return () => {
  //     unlistenPromise.then((unlisten) => unlisten());
  //   };
  // }, []);
  const setSelected = () => {
        props.setSelected(props);
        console.log(props.tags.Title);
      }
  return(
    <div
      onContextMenu={ setSelected }
      onClick={ setSelected }
      className="song">
      <p className="artist unselectable">{ props.tags.Artist }</p>
      <p className="title  unselectable">{ props.tags.Title }</p>
      <p className="album  unselectable">{ props.tags.Album }</p>
      <p className="duration  unselectable">
        { Math.round(+props.duration / 60) }:
        { (+props.duration % 60).toString().padStart(2, "0") }
      </p>
    </div>
  )
}
