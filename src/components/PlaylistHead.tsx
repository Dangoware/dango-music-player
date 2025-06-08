import { MutableRefObject, useEffect } from "react"
import { MainViewSong, SongProps } from "./MainView"
import { invoke } from "@tauri-apps/api/core"
import { Song } from "../bindings/Song"
import { appWindow, contextMenuPosition } from "./App"
import { Menu } from "@tauri-apps/api/menu"

export interface PlaylistInfo {
  uuid: string,
  name: string,
}


export interface PlaylistHeadProps {
  playlists: JSX.Element[]
  setPlaylists: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
  setViewName: React.Dispatch<React.SetStateAction<string>>,
  setLibrary: React.Dispatch<React.SetStateAction<JSX.Element[]>>,
  playlistsInfo: MutableRefObject<PlaylistInfo[]>,
  setSelected: (props: SongProps) => void,
 }

export function PlaylistHead({ playlists, setPlaylists, setViewName, setLibrary, playlistsInfo, setSelected }: PlaylistHeadProps) {
  function getPlaylist(playlist: PlaylistInfo) {
    invoke('get_playlist', { uuid: playlist.uuid }).then((list) => {
      setLibrary([...(list as Song[]).map((song, i) => {
        // console.log(song);
        const reload = () => getPlaylist(playlist)
        return (
          <MainViewSong
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
    const unlisten = appWindow.listen<PlaylistInfo[]>("playlists_gotten", (_res) => {
      const res = _res.payload;
      // console.log(event);
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
      let res = _res as PlaylistInfo;

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
          setLibrary([...(lib as Song[]).map((song) => {
            // console.log(song);
            i++;
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
