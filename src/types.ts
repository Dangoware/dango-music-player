export interface Configlibrary {
    name: string,
    path: string,
    uuid: string,
    scan_folders?: string[]
}

export interface ConfigLibraries {
    default_library: string,
    library_folder: string,
    libraries: Configlibrary[]
}

export interface Config {
    path: string,
    backup_folder?: string,
    libraries: ConfigLibraries,
    volume: number,
    connections: ConfigConnections,
}

export interface ConfigConnections {
    listenbrainz_token?: string
}

export interface Song {
    location: URI[],
    uuid: string,
    plays: number,
    skips: number,
    favorited: boolean,
    banned?: BannedType,
    rating?: number,
    format?: string,
    duration: number,
    play_time: number,
    last_played?: number,
    date_added?: number,
    date_modified?: number,
    album_art: AlbumArt[],
    tags: Map<Tag, String>,
    internal_tags: InternalTag[],
}

export enum InternalTag {

}

export enum Tag {

}

export enum AlbumArt {

}

export enum URI {

}

export enum BannedType {

}

export interface playbackInfo {
    position?: [number, number],
    duration?: [number, number],
}