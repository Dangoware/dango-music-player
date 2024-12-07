import { useEffect, useRef, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Config, Song } from "./types";

import { decode, encode } from 'cbor-x';
import CBOR from "cbor";

function App() {

  useEffect(() => {
    getConfig();
    invoke('set_volume', { volume: 0.04 }).then( () => {} )
  }, [])


  return (
    <main className="container">
      <div className="leftSide">
        <PlaylistHead />
        <MainView />
        <PlayBar />
      </div>
      <div className="rightSide">
        <NowPlaying />
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

function MainView() {
  return (
    <div className="mainView">
      main view
      <button onClick={ () => invoke('get_library').then((bytes) => {
        console.log(bytes);
        let arr = new Uint8Array(bytes as ArrayBuffer);
        let a: any = CBOR.decode(arr);
        console.log(a);

      }) }>get library</button>
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
        <input type="range" name="volume" id="volumeSlider" />
      </div>
      <input type="range" name="seek" id="seekBar" />
    </section>
  )
}

function NowPlaying() {
  return (
    <section className="nowPlaying">
      <img id="nowPlayingArtwork" src="https://images.genius.com/22648cfc4f618884df6d6082962b34d2.1000x1000x1.png" />
      <h2>やけにインザレイン</h2>
      <p>t+pazolite; 小林私</p>
      <p>Heartache Debug</p>
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