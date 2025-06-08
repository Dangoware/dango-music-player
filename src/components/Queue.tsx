import { MutableRefObject } from "react";
import { PlayerLocation } from "../bindings/PlayerLocation";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Menu } from "@tauri-apps/api/menu";
import { contextMenuPosition } from "./App";
import { Song } from "../bindings/Song";

export interface QueueProps {
  songs: JSX.Element[],
  // song element. put the proper type here :]???
  selectedSong: MutableRefObject<selectedQueueSong>,
}

export interface selectedQueueSong {
  uuid: string,
  index: number
  location: PlayerLocation,
}

export function Queue({ songs, selectedSong, }: QueueProps) {
   const removeFromQueue = () => {
      invoke('remove_from_queue', { index: selectedSong.current.index }).then(() => {});
    }
    const playNow = () => {
      invoke('queue_move_to', { index: selectedSong.current.index }).then(() => {});
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

export interface QueueSongProps {
  song: Song,
  location: PlayerLocation,
  index: number,
  setSelectedSong: (song: selectedQueueSong) => void,
}

export function QueueSong({ song, location, index, setSelectedSong }: QueueSongProps) {
  // console.log(song.tags);

  let setSelected = () => setSelectedSong({uuid: song.uuid, index: index, location: location })

  return (
    <div className="queueSong unselectable"  onAuxClickCapture={ setSelected } onClick={ setSelected } onContextMenu={ setSelected }>
      <img className="queueSongCoverArt" src={ convertFileSrc('abc') + '?' + song.uuid } key={ 'coverArt_' + song.uuid }/>
      <div className="queueSongTags">
        <p className="queueSongTitle">{ song.tags.Title }</p>
        <p className="queueSongArtist">{ song.tags.Artist }</p>
      </div>
    </div>
  )
}
