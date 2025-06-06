import React, { MutableRefObject, useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Config, playbackInfo } from "./types";
// import { EventEmitter } from "@tauri-apps/plugin-shell";
// import { listen } from "@tauri-apps/api/event";
// import { fetch } from "@tauri-apps/plugin-http";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { cursorPosition, LogicalPosition, PhysicalPosition } from "@tauri-apps/api/window";
import { Menu, Submenu, SubmenuOptions } from "@tauri-apps/api/menu";
import { TauriEvent } from "@tauri-apps/api/event";


const appWindow = getCurrentWebviewWindow();

type Location = "Library" | { "Playlist": string };

// This needs to be changed to properly reflect cursor position
// this will do for now.
async function contextMenuPosition(event: React.MouseEvent)  {
  return new PhysicalPosition(event.clientX, event.clientY);
}

function App() {
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
    const unlisten = appWindow.listen<any>("now_playing_change", ({ payload, }) => {
        const displayArtwork = () => {
          invoke('display_album_art', { uuid: payload.uuid }).then(() => {})
        }
        // console.log(event);
        setNowPlaying(
          <NowPlaying
            title={ payload.tags.TrackTitle }
            album={ payload.tags.AlbumTitle }
            artist={ payload.tags.TrackArtist }
            artwork={ <img src={convertFileSrc("abc") + "?" + payload.uuid } id="nowPlayingArtwork" alt="Now Playing Artwork" key={payload.uuid} onDoubleClick={ displayArtwork } /> }
          />
        )

    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  useEffect(() => {
    const unlisten = appWindow.listen<any>("queue_updated", (_) => {
        // console.log(event);
        invoke('get_queue').then((_songs) => {
          let songs = _songs as any[];
            setQueue(
              songs.filter((_, i) => i != 0).map((song, i) =>
                <QueueSong
                  song={ song[0] }
                  location={ song[1] as "Library" | {"Playlist" : string}}
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
    const unlisten = appWindow.listen<any>("playing", (isPlaying) => {
        setPlaying(isPlaying.payload as boolean)
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

export default App;

interface PlaylistInfo {
  uuid: string,
  name: string,
}


interface PlaylistHeadProps {
  playlists: JSX.Element[]
  setPlaylists: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
  setViewName: React.Dispatch<React.SetStateAction<string>>,
  setLibrary: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
  playlistsInfo: MutableRefObject<PlaylistInfo[]>,
  setSelected: (props: SongProps) => void,
 }

function PlaylistHead({ playlists, setPlaylists, setViewName, setLibrary, playlistsInfo, setSelected }: PlaylistHeadProps) {
  function getPlaylist(playlist: PlaylistInfo) {
    invoke('get_playlist', { uuid: playlist.uuid }).then((list) => {      
      setLibrary([...(list as any[]).map((song, i) => {
        // console.log(song);
        const reload = () => getPlaylist(playlist)
        return (
          <Song
            key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
            location={ song.location }
            playerLocation={ {"Playlist" : playlist.uuid } }
            uuid={ song.uuid }
            plays={ song.plays }
            duration={ song.duration }
            tags={ song.tags }
            playlists={ playlistsInfo }
            reload = { reload }
            index = { i }
            setSelected={ setSelected }
          />
        )
      })])
    })
    setViewName( playlist.name )
  }

  useEffect(() => {
    const unlisten = appWindow.listen<any[]>("playlists_gotten", (_res) => {
        // console.log(event);
        let res = _res.payload as PlaylistInfo[];
        playlistsInfo.current = [...res];
        // console.log(playlistsInfo, res);

        setPlaylists([
          ...res.map( (list) => {
            const _getPlaylist = () => getPlaylist(list)
            const deletePlaylist = () => {
              invoke('delete_playlist', { uuid: list.uuid }).then(() => {});
              invoke('get_playlists').then(() => {});
            }
            async function menuHandler(event: React.MouseEvent) {
              event.preventDefault();
              const menu = await Menu.new({
                items: [
                  { id: "delete_playlist" + list.uuid, text: "Delete Playlist", action: deletePlaylist }
                ]
              });
              menu.popup(await contextMenuPosition(event));
            }

            return (
              <button onClick={ _getPlaylist }
                onContextMenu={ menuHandler }
                 key={ 'playlist_' + list.uuid }>{ list.name }</button>
            )
          })
        ])
    })
    return () => { unlisten.then((f) => f()) }
  }, []);
    let handle_import = () => {
    invoke('import_playlist').then((_res) => {
      let res = _res as any;

      setPlaylists([
        ...playlists,
        <button onClick={ () => getPlaylist(res) } key={ 'playlist_' + res.uuid }>{ res.name }</button>
      ])
      // console.log(res.name);
    })
  }
  return (
    <section className="playlistHead">
      <button onClick={() => {
        setViewName("Library");
        invoke('get_library').then((lib) => {
          let i = 0;
          setLibrary([...(lib as any[]).map((song) => {
            // console.log(song);
            i++;
            return (
              <Song
                key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
                location={ song.location }
                playerLocation="Library"
                uuid={ song.uuid }
                plays={ song.plays }
                duration={ song.duration }
                tags={ song.tags }
                playlists={ playlistsInfo }
                index={ i - 1 }
                setSelected={ setSelected }
              />
            )
          })])
        })
      } }>Library</button>
        { playlists }
      <button onClick={ handle_import }>Import .m3u Playlist</button>
      <button onClick={() => { invoke('open_config_window').then(() => {}) }} style={{marginLeft: "auto", float: "right"}}>Edit DMP</button>
    </section>
  )
}

interface MainViewProps {
  lib_ref: [JSX.Element[], React.Dispatch<React.SetStateAction<JSX.Element[]>>],
  viewName: string,
  playlistsInfo: MutableRefObject<PlaylistInfo[]>,
  setSelected: (props: SongProps) => void,
  selectedSong: MutableRefObject<SongProps | undefined>,
}

function MainView({ lib_ref, viewName, playlistsInfo, setSelected, selectedSong }: MainViewProps) {
  const [library, setLibrary] = lib_ref;
  

  const addToQueue = (_: string) => {
    invoke('add_song_to_queue', { uuid: selectedSong.current!.uuid, location: selectedSong.current!.playerLocation }).then(() => {});
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
            } as SubmenuOptions
          ),
        { id: "remove_from_lib_playlist" + selectedSong.current!.location + selectedSong.current!.uuid, text: removeText, action: removeLibPlaylist },
      ]});
      menu.popup(await contextMenuPosition(event));
    }


  useEffect(() => {
    const unlisten = appWindow.listen<any>("library_loaded", (_) => {
      console.log("library_loaded");
      invoke('get_library').then((lib) => {
        let i = 0;
        setLibrary([...(lib as any[]).map((song) => {
          i++;
          return (
            <Song
              key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
              location={ song.location }
              playerLocation="Library"
              uuid={ song.uuid }
              plays={ song.plays }
              duration={ song.duration }
              tags={ song.tags }
              playlists={ playlistsInfo }
              index={ i - 1 }
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


interface SongProps {
  location: any,
  playerLocation: string | {"Playlist" : any},
  uuid: string,
  plays: number,
  format?: string,
  duration: string,
  last_played?: string,
  date_added?: string,
  date_modified?: string,
  tags: any,
  playlists: MutableRefObject<PlaylistInfo[]>,
  index: number,
  setSelected: (props: SongProps) => void,
  reload?: () => void
}

function Song(props: SongProps) {
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
        console.log(props.tags.TrackTitle);
      }
  return(
    <div
      onContextMenu={ setSelected }
      onClick={ setSelected }
      className="song">
      <p className="artist unselectable">{ props.tags.TrackArtist }</p>
      <p className="title  unselectable">{ props.tags.TrackTitle }</p>
      <p className="album  unselectable">{ props.tags.AlbumTitle }</p>
      <p className="duration  unselectable">
        { Math.round(+props.duration / 60) }:
        { (+props.duration % 60).toString().padStart(2, "0") }
      </p>
    </div>
  )
}


interface PlayBarProps {
  playing: boolean,
  setPlaying: React.Dispatch<React.SetStateAction<boolean>>
}

function PlayBar({ playing, setPlaying }: PlayBarProps) {
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const [seekBarSize, setSeekBarSize] = useState(0);
  const seekBarRef = React.createRef<HTMLDivElement>();

  const [lastFmLoggedIn, setLastFmLoggedIn] = useState(false);

  useEffect(() => {
    const unlisten = appWindow.listen<any>("playback_info", ({ payload, }) => {
      const info = payload as playbackInfo;
      const pos_ = Array.isArray(info.position) ? info.position![0] : 0;
      const dur_ = Array.isArray(info.duration) ? info.duration![0] : 0;

      setPosition(pos_);
      setDuration(dur_);
      let progress = ((pos_/dur_) * 100);
      setSeekBarSize(progress)
    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  const seek = (event: React.MouseEvent<HTMLDivElement>) => {
    event.stopPropagation();
    let rect = seekBarRef.current!.getBoundingClientRect();
    let val = ((event.clientX-rect.left) / (rect.width))*duration;

    invoke('seek', { time: Math.round(val * 1000) }).then()
  };

  return (
    <section id="playBar" className="playBar unselectable">
      <div className="seekBar" ref={ seekBarRef } onClick={ seek } onDrag={ seek }>
        <div className="seekOverlay" id="seekOverlay" style={{ width: seekBarSize + '%' } }></div>
      </div>
      <div className="bottomSpaced">
        <div className="bottomLeft">
          <button onClick={ (_) => {
            setPlaying( playing ? false : true );
            invoke( playing ? 'pause' : 'play' ).then(() => {})
          }}>{ playing ? '⏸' : '⏵' }</button>
          <button onClick={ () => invoke('stop').then(() => {}) }>⏹</button>
          <button onClick={ () => invoke('prev').then(() => {}) }>⏮</button>
          <button onClick={ () => invoke('next').then(() => {}) }>⏭</button>
        </div>
        <div className="bottomRight">
          <button>🔀</button>
          <button>🔁</button>
          <input type="range" name="volume" id="volumeSlider" onChange={ (volume) => {
            invoke('set_volume', { volume: volume.target.value }).then(() => {})
          }} />
          <p id="timeDisplay">
            { Math.floor(+position / 60).toString().padStart(2, "0") }:
            { (+position % 60).toString().padStart(2, "0") }/
            { Math.floor(+duration / 60).toString().padStart(2, "0") }:
            { (+duration % 60).toString().padStart(2, "0") }

          </p>
        </div>
      </div>
    </section>
  )
}

interface NowPlayingProps {
  title: string,
  artist: string,
  album: string,
  artwork: JSX.Element
}

function NowPlaying({ title, artist, album, artwork }: NowPlayingProps) {
  return (
    <section className="nowPlaying">
      <div className="artworkWrapper unselectable">
        { artwork }
      </div>
      <h3>{ title }</h3>
      <p>{ artist }</p>
      <p>{ album }</p>
    </section>
  )
}

interface QueueProps {
  songs: JSX.Element[],
  // song element. put the proper type here :]???
  selectedSong: MutableRefObject<selectedQueueSong>,
}

interface selectedQueueSong {
  uuid: string,
  index: number
  location: Location,
}

function Queue({ songs, selectedSong, }: QueueProps) {
   const removeFromQueue = () => {
        invoke('remove_from_queue', { index: selectedSong.current.index }).then(() => {})
      }
      const playNow = () => {
        invoke('play_now', { uuid: selectedSong.current.uuid, location: selectedSong.current.location }).then(() => {})
      }
      const playNext = () => invoke('play_next_queue', { uuid: selectedSong.current.uuid, location: selectedSong.current.location }).then(() => {});
      const clearQueue = () => invoke('clear_queue').then();

      async function menuHandler(event: React.MouseEvent) {
        event.preventDefault();

        const menu = await Menu.new({
         items: [
           { id: "play_now" + selectedSong.current.index, text: "Play Now", action: playNow },
           { id: "play_next_" + selectedSong.current.uuid + selectedSong.current.index, text: "Play Next in Queue", action: playNext },
           { id: "remove_queue" + selectedSong.current.uuid + selectedSong.current.index, text: "Remove from Queue", action: removeFromQueue },
           { id: "clear_queue", text: "Clear Queue", action: clearQueue },
         ]
        })
        menu.popup(await contextMenuPosition(event));
      }

  return (
    <section className="Queue"
      onAuxClickCapture={ removeFromQueue }
      onDoubleClick={ playNow }
      onContextMenu={ menuHandler }
      >
      { songs }
    </section>
  )
}

interface QueueSongProps {
  song: any,
  location: Location,
  index: number,
  setSelectedSong: (song: selectedQueueSong) => void,
}

function QueueSong({ song, location, index, setSelectedSong }: QueueSongProps) {
  // console.log(song.tags);

  let setSelected = () => setSelectedSong({uuid: song.uuid, index: index, location: location })

  return (
    <div className="queueSong unselectable"  onAuxClickCapture={ setSelected } onClick={ setSelected } onContextMenu={ setSelected }>
      <img className="queueSongCoverArt" src={ convertFileSrc('abc') + '?' + song.uuid } key={ 'coverArt_' + song.uuid }/>
      <div className="queueSongTags">
        <p className="queueSongTitle">{ song.tags.TrackTitle }</p>
        <p className="queueSongArtist">{ song.tags.TrackArtist }</p>
      </div>
    </div>
  )
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

