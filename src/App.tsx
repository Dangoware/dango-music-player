import { useEffect, useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { Config, Song } from "./types";

function App() {

  useEffect(() => {
    getConfig();
    invoke('set_volume', { volume: 0.02 }).then( () => {} )
  }, [])


  return (
    <main className="container">
      <button onClick={ () => newWindow() }>load config/library</button>
      <button onClick={ () => {
        invoke('get_library').then((library) => {
          console.log(library as Song[]);
        })
      }}>get library</button>
      <button onClick={ () => invoke('play').then(() => {}) }>play</button>
      <button onClick={ () => invoke('pause').then(() => {}) }>pausers</button>
      <button onClick={ () => invoke('next').then(() => {}) }>next</button>
      <button onClick={ () => invoke('prev').then(() => {}) }>back</button>
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

interface LibProps {

}

{/* function Library() {
  let [library, setLibrary] = useState<any>()
  useEffect( () => {

  }, [])

  return (
    <>
      { library }
    </>
  )
} */}