import React, { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "./App.css";
// import { EventEmitter } from "@tauri-apps/plugin-shell";
// import { listen } from "@tauri-apps/api/event";
// import { fetch } from "@tauri-apps/plugin-http";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { PhysicalPosition } from "@tauri-apps/api/window";
import { Config } from "../bindings/Config";
import { PlayerLocation } from "../bindings/PlayerLocation";
import { Song } from "../bindings/Song";
import { PlaylistHead, PlaylistInfo } from "./PlaylistHead";
import MainView, { SongProps } from "./MainView";
import { Queue, QueueSong, selectedQueueSong } from "./Queue";
import { NowPlaying } from "./NowPlaying";
import { PlayBar } from "./PlayBar";

export const appWindow = getCurrentWebviewWindow();

// This needs to be changed to properly reflect cursor position
// this will do for now.
export async function contextMenuPosition(event: React.MouseEvent)  {
  return new PhysicalPosition(event.clientX, event.clientY);
}

export default function App() {
  const library = useState<JSX.Element[]>([]);
  const [queue, setQueue] = useState<JSX.Element[]>([]);
  const [playing, setPlaying] = useState(false);
  const [playlists, setPlaylists] = useState<JSX.Element[]>([]);
  const [viewName, setViewName] = useState("Library");
  const playlistsInfo= useRef<PlaylistInfo[]>([]);
  const selectedSongMain = useRef<SongProps>();
  const selectedSongQueue = useRef<selectedQueueSong>({uuid: "0", index: 0, location: "Library"});
  const setSelectedSongMain = (props: SongProps) =>  {selectedSongMain.current = props;}
  const setSelectedSongQueue = (song: selectedQueueSong) => {selectedSongQueue.current = song; console.log(selectedSongQueue)}

  const [nowPlaying, setNowPlaying] = useState<JSX.Element>(
    <NowPlaying
      title="Title"
      album="Album"
      artist="Artist"
      artwork={<img src={convertFileSrc("abc") + "?" + "default" } id="nowPlayingArtwork" alt="Now Playing Artwork" key={'default_image'} />}
    />
  );


  useEffect(() => {
    const unlisten = appWindow.listen<Song>("now_playing_change", ({ payload, }) => {
        const displayArtwork = () => {
          invoke('display_album_art', { uuid: payload.uuid }).then(() => {})
        }
                
        setNowPlaying(
          <NowPlaying
            title={ payload.tags.Title }
            album={ payload.tags.Album }
            artist={ payload.tags.Artist }
            artwork={ <img src={convertFileSrc("abc") + "?" + payload.uuid } id="nowPlayingArtwork" alt="Now Playing Artwork" key={payload.uuid} onDoubleClick={ displayArtwork } /> }
          />
        )

    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  useEffect(() => {
    const unlisten = appWindow.listen<null>("queue_updated", (_) => {
        // console.log(event);
        invoke('get_queue').then((_songs) => {
          let songs = _songs as [Song, PlayerLocation][];
            setQueue(
              songs.filter((_, i) => i != 0).map((song, i) =>
                <QueueSong
                  song={ song[0] }
                  location={ song[1] }
                  index={i+1}
                  key={ Math.floor((Math.random() * 100_000_000_000) + 1) + '_' + Date.now() }
                  setSelectedSong={ setSelectedSongQueue }
                />
              )
            )
        })
    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  useEffect(() => {
    const unlisten = appWindow.listen<boolean>("playing", (isPlaying) => {
        setPlaying(isPlaying.payload)
    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  useEffect(() => {
    invoke("start_controller").then(() => {});
  }, [])


  return (
    <main>
      <div className="container">
        <div className="leftSide">
          <PlaylistHead playlists={ playlists } setPlaylists={ setPlaylists } setViewName={ setViewName } setLibrary={ library[1] } playlistsInfo={ playlistsInfo } setSelected={ setSelectedSongMain } />
          <MainView lib_ref={ library } viewName={ viewName } playlistsInfo={ playlistsInfo } setSelected={ setSelectedSongMain } selectedSong={ selectedSongMain} />
        </div>
        <div className="rightSide">
          { nowPlaying }
          <Queue songs={ queue } selectedSong={ selectedSongQueue } />
        </div>
      </div>
      <div className="bottom">
        <PlayBar playing={ playing } setPlaying={ setPlaying } />
      </div>
    </main>
  );
}

function getConfig(): any {
  invoke('init_get_config').then( (_config) => {
    let config = _config as Config;
    if (config.libraries.libraries.length == 0) {
      invoke('create_new_library').then(() => {})
    } else {
      // console.log("else");
      invoke('lib_already_created').then(() => {})
    }
  })
}

