import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Config } from "./types";
import { EventEmitter } from "@tauri-apps/plugin-shell";
import { listen } from "@tauri-apps/api/event";

function App() {
  const library = useState<JSX.Element[]>();
  const [artwork, setArtwork] = useState<JSX.Element>(<></>);
  const [nowPlaying, setNowPlaying] = useState<JSX.Element>(<NowPlaying title="blank" album="blank" artist="blank" artwork={ artwork }/>);

  listen<any>("now_playing_change", (event) => {
    console.log(event.payload);

    setNowPlaying( <NowPlaying
      title={ event.payload.tags.TrackTitle }
      album={ event.payload.tags.AlbumTitle }
      artist={ event.payload.tags["DISPLAY ARTIST"]}
      artwork={ artwork } />)
      setArtwork( <img src="asset://localhost" id="nowPlayingArtwork" /> )
  })

  useEffect(() => {
    getConfig();
    invoke('set_volume', { volume: "1" }).then( () => {} )
  }, [])

  return (
    <main className="container">
      <div className="leftSide">
        <PlaylistHead />
        <MainView lib_ref={ library } />
        <PlayBar />
      </div>
      <div className="rightSide">
        { nowPlaying }
        <Queue />
      </div>

    </main>
  );
}

export default App;

function getConfig(): any {
  invoke('get_config').then( (_config) => {
    let config = _config as Config;
    if (config.libraries.libraries.length == 0) {
      newWindow()
    } else {
      console.log("else");
      invoke('lib_already_created').then(() => {})
    }
  })
}

function newWindow() {
  invoke('new_library_window').then(() => {})
}

function PlaylistHead() {
  return (
    <section className="playlistHead">
      <button>Library</button>
      <button>Playlist 1</button>
      <button>Playlist 2</button>
      <button>Playlist 3</button>
      <button>Playlist 4</button>
      <button>Playlist 5</button>
      <button>Playlist 6</button>
    </section>
  )
}

interface MainViewProps {
  lib_ref: [JSX.Element[] | undefined, React.Dispatch<React.SetStateAction<JSX.Element[] | undefined>>],
}

function MainView({ lib_ref }: MainViewProps) {
  const [library, setLibrary] = lib_ref;
  // useEffect(() => {
  //   console.log(lib_ref);
  //   console.log(typeof lib_ref);
  //   if (typeof lib_ref.current !== "undefined") {

  //     setLibrary(lib_ref.current.map((song) => {
  //         <Song
  //         location={ song.location }
  //         uuid={ song.uuid }
  //         plays={ song.plays }
  //         duration={ song.duration }
  //         tags={ song.tags }
  //         />
  //     }))
  //   }

  // }, [lib_ref])

  return (
    <div className="mainView">
      main view
      <button onClick={ (e) => {
        e.preventDefault();

        invoke('get_library').then((lib) => {
          setLibrary([...(lib as any[]).map((song) => {
            console.log(song);

            return (
              <Song
                key={ song.uuid }
                location={ song.location }
                uuid={ song.uuid }
                plays={ song.plays }
                duration={ song.duration }
                tags={ song.tags }
              />
            )
          })])
      })} }>get library</button>
      <div>{ library }</div>
    </div>
  )
}

interface SongProps {
  location: any,
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
  console.log(props.tags);

  return(
    <div className="song">
      <p className="title">{ props.tags.TrackTitle }</p>
      <p className="album">{ props.tags.Album }</p>
      <p className="artist">{ props.tags.AlbumArtist }</p>
      <p className="duration">{ props.duration }</p>
    </div>
  )
}

function PlayBar() {
  let [playing, setPlaying] = useState('play');
  return (
    <section id="playBar" className="playBar">
      <div className="topHalf">
        <div>
          <button>shuffle</button>
          <button>loop</button>
        </div>
        <button onClick={ () => invoke('prev').then(() => {}) }>prev</button>
        <button onClick={ (_) => {
          if (playing == 'play') {
            setPlaying('pause')
            invoke('play').then(() => {})
          } else {
            setPlaying('play')
            invoke('pause').then(() => {})
          }
        }}>{ playing }</button>
        <button onClick={ () => invoke('next').then(() => {}) }>next</button>
        <input type="range" name="volume" id="volumeSlider" onChange={ (volume) => {
          invoke('set_volume', { volume: volume.target.value }).then(() => {})
        }} />
      </div>
      <input type="range" name="seek" id="seekBar" />
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
  console.log(convertFileSrc("abc"));

  return (
    <section className="nowPlaying">
      { artwork }
      <h2>{ title }</h2>
      <p>{ artist }</p>
      <p>{ album }</p>
    </section>
  )
}

function Queue() {
  return (
    <section className="Queue">
      This is where the Queue be
    </section>
  )
}