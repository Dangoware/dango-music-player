import React, { createRef, ReactEventHandler, useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Config, playbackInfo } from "./types";
// import { EventEmitter } from "@tauri-apps/plugin-shell";
// import { listen } from "@tauri-apps/api/event";
// import { fetch } from "@tauri-apps/plugin-http";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
import { Menu } from "@tauri-apps/api/menu";
import { listen } from "@tauri-apps/api/event";

const appWindow = getCurrentWebviewWindow();

function App() {
  const library = useState<JSX.Element[]>([]);
  const [queue, setQueue] = useState<JSX.Element[]>([]);
  const [playing, setPlaying] = useState(false);
  const [playlists, setPlaylists] = useState<JSX.Element[]>([]);
  const [viewName, setViewName] = useState("Library");

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
    const invoke_: any = invoke("start_controller").then(() => {});
  }, [])

  return (
    <main>
      <div className="container">
        <div className="leftSide">
          <PlaylistHead playlists={ playlists } setPlaylists={ setPlaylists } setViewName={ setViewName } setLibrary={ library[1] } />
          <MainView lib_ref={ library } viewName={ viewName } />
        </div>
        <div className="rightSide">
          { nowPlaying }
          <Queue songs={ queue } />
        </div>
      </div>
      <div className="bottom">
        <PlayBar playing={ playing } setPlaying={ setPlaying } />
      </div>
    </main>
  );
}

export default App;

interface PlaylistHeadProps {
  playlists: JSX.Element[]
  setPlaylists: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
  setViewName: React.Dispatch<React.SetStateAction<string>>,
  setLibrary: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
}

function PlaylistHead({ playlists, setPlaylists, setViewName, setLibrary }: PlaylistHeadProps) {

  useEffect(() => {
    const unlisten = appWindow.listen<any[]>("playlists_gotten", (_res) => {
        // console.log(event);
        let res = _res.payload;

        setPlaylists([
          ...res.map( (item) => {
            return (
              <button onClick={ () => {
                invoke('get_playlist', { uuid: item.uuid }).then((list) => {
                setLibrary([...(list as any[]).map((song) => {
                  // console.log(song);
                  return (
                    <Song
                      key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
                      location={ song.location }
                      playerLocation={ {"Playlist" : item.uuid } }
                      uuid={ song.uuid }
                      plays={ song.plays }
                      duration={ song.duration }
                      tags={ song.tags }
                    />
                  )
                  })])
                })
                setViewName( item.name )
              } } key={ 'playlist_' + item.uuid }>{ item.name }</button>
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
        <button onClick={ () => {
          invoke('get_playlist', { uuid: res.uuid }).then((list) => {
            console.log((list as any[]).length);

            setLibrary([...(list as any[]).map((song) => {
              // console.log(song);
              return (
                <Song
                  key={ song.uuid }
                  location={ song.location }
                  playerLocation={ {"Playlist" : res.uuid } }
                  uuid={ song.uuid }
                  plays={ song.plays }
                  duration={ song.duration }
                  tags={ song.tags }
                />
              )
            })])
          })
          setViewName( res.name )
        } } key={ 'playlist_' + res.uuid }>{ res.name }</button>
      ])
      console.log(res.name);
    })
  }
  return (
    <section className="playlistHead">
      <button onClick={() => {
        setViewName("Library");
        invoke('get_library').then((lib) => {
          setLibrary([...(lib as any[]).map((song) => {
            console.log(song);

            return (
              <Song
                key={ song.uuid }
                location={ song.location }
                playerLocation="Library"
                uuid={ song.uuid }
                plays={ song.plays }
                duration={ song.duration }
                tags={ song.tags }
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
  viewName: string
}

function MainView({ lib_ref, viewName }: MainViewProps) {
  const [library, setLibrary] = lib_ref;

  useEffect(() => {
    const unlisten = appWindow.listen<any>("library_loaded", (_) => {
      console.log("library_loaded");
      invoke('get_library').then((lib) => {
        setLibrary([...(lib as any[]).map((song) => {

          return (
            <Song
              key={ song.uuid + Math.floor(Math.random() * 100_000_000_000) }
              location={ song.location }
              playerLocation="Library"
              uuid={ song.uuid }
              plays={ song.plays }
              duration={ song.duration }
              tags={ song.tags }
            />
          )
        })])
      })

      invoke('get_playlists').then(() => {})
    })
    return () => { unlisten.then((f) => f()) }
  }, []);


  return (
    <div className="mainView">
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
  tags: any
}

function Song(props: SongProps) {
  // console.log(props.tags);
  const add_to_queue_test = (_: string) => {
    invoke('add_song_to_queue', { uuid: props.uuid, location: props.playerLocation }).then(() => {});
  }

  const songMenuPromise = Menu.new({
    items: [
      { id: "add_song_to_queue" + props.uuid, text: "Add to Queue", action: add_to_queue_test}
    ]
  })

  async function clickHandler(event: React.MouseEvent) {
    event.preventDefault();
    const menu = await songMenuPromise;
    const pos = new LogicalPosition(event.clientX, event.clientY);
    menu.popup(pos);
  }
 
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

  return(
    <div
      onDoubleClick={() => {
        invoke("play_now", { uuid: props.uuid, location: props.playerLocation }).then(() => {})
      }}
       onContextMenu={clickHandler}
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
}

function Queue({ songs }: QueueProps) {
  return (
    <section className="Queue">
      { songs }
    </section>
  )
}

interface QueueSongProps {
  song: any,
  location: "Library" | {"Playlist": string},
  index: number,
}

function QueueSong({ song, location, index }: QueueSongProps) {
  // console.log(song.tags);

  let removeFromQueue = () => {
    invoke('remove_from_queue', { index: index }).then(() => {})
  }

  let playNow = () => {
    invoke('play_now', { uuid: song.uuid, location: location }).then(() => {})
  }

  return (
    <div className="queueSong unselectable"  onAuxClickCapture={ removeFromQueue } onDoubleClickCapture={ playNow }>
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
