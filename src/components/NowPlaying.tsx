export interface NowPlayingProps {
  title: string | undefined,
  artist: string | undefined,
  album: string | undefined,
  artwork: JSX.Element
}

export function NowPlaying({ title, artist, album, artwork }: NowPlayingProps) {
  return (
    <section className="nowPlaying">
      <div className="artworkWrapper unselectable">
        { artwork }
      </div>
      <h3>{ title? title : "Unknown Title" }</h3>
      <p>{ artist }</p>
      <p>{ album }</p>
    </section>
  )
}
