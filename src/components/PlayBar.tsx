import React, { useEffect, useRef, useState } from "react";
import { appWindow } from "./App";
import { PlaybackInfo } from "../bindings/PlaybackInfo";
import { invoke } from "@tauri-apps/api/core";

export interface PlayBarProps {
  playing: boolean,
  setPlaying: React.Dispatch<React.SetStateAction<boolean>>
}

export function PlayBar({ playing, setPlaying }: PlayBarProps) {
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const [seekBarSize, setSeekBarSize] = useState(0);
  const shuffled = useRef(false);
  const seekBarRef = React.createRef<HTMLDivElement>();
  const volumeSliderRef = React.createRef<HTMLInputElement>();

  const [lastFmLoggedIn, setLastFmLoggedIn] = useState(false);

  useEffect(() => {
    const unlisten = appWindow.listen<PlaybackInfo>("playback_info", ({ payload: info, }) => {
      const pos_ = Array.isArray(info.position) ? info.position![0] : 0;
      const dur_ = Array.isArray(info.duration) ? info.duration![0] : 0;

      setPosition(pos_);
      setDuration(dur_);
      let progress = ((pos_ / dur_) * 100);
      setSeekBarSize(progress)
    })
    return () => { unlisten.then((f) => f()) }
  }, []);

  const seek = (event: React.MouseEvent<HTMLDivElement>) => {
    event.stopPropagation();
    let rect = seekBarRef.current!.getBoundingClientRect();
    let val = ((event.clientX - rect.left) / (rect.width)) * duration;

    invoke('seek', { time: Math.round(val * 1000) }).then()
  };

  const wheelVolume = (event: React.WheelEvent<HTMLDivElement>) => {
    const n = 5;
    if (event.deltaY < 0) {
      volumeSliderRef.current!.stepUp(n);
    } else {
      volumeSliderRef.current!.stepDown(n);
    }
    invoke('set_volume', { volume: volumeSliderRef.current?.valueAsNumber!.toString() }).then(() => { })
  };


  const toggle_shuffle = () => {
    invoke("queue_shuffle", { shuffle: !shuffled.current }).then(() => {
      shuffled.current = !shuffled.current;
    })
  }

  return (
    <section id="playBar" className="playBar unselectable">
      <div className="seekBar" ref={seekBarRef} onClick={seek} onDrag={seek}>
        <div className="seekOverlay" id="seekOverlay" style={{ width: seekBarSize + '%' }}></div>
      </div>
      <div className="bottomSpaced">
        <div className="bottomLeft">
          <button onClick={(_) => {
            setPlaying(playing ? false : true);
            invoke(playing ? 'pause' : 'play').then(() => { })
          }}>{playing ? '⏸' : '⏵'}</button>
          <button onClick={() => invoke('stop').then(() => { })}>⏹</button>
          <button onClick={() => invoke('prev').then(() => { })}>⏮</button>
          <button onClick={() => invoke('next').then(() => { })}>⏭</button>
        </div>
        <div className="bottomRight">
          <button onClick={toggle_shuffle} >🔀</button>
          <button>🔁</button>
          <input ref={volumeSliderRef} onWheel={wheelVolume} type="range" name="volume" id="volumeSlider" onChange={(volume) => {
            invoke('set_volume', { volume: volume.target.value }).then(() => { })
          }} />
          <p id="timeDisplay">
            {Math.floor(+position / 60).toString().padStart(2, "0")}:
            {(+position % 60).toString().padStart(2, "0")}/
            {Math.floor(+duration / 60).toString().padStart(2, "0")}:
            {(+duration % 60).toString().padStart(2, "0")}

          </p>
        </div>
      </div>
    </section>
  )
}
