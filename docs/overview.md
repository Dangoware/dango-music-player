# Core Architecture:

Dango music player operates with a standalone cross-platform backend that can function with multiple frontends

This is to be implemented such that the `dango-core` crate controls all music-related functions, and provides an api for frontends to use.

`dango-gui` is intended to be a fully featured cross-platform native modular frontend

`dango-cli` is intended to be a minimal cli interface with basic playback and playlist support

---
###### note: all structure is subject to change during development

## dango-core:

Core functionality is provided by the `MusicController` struct, which provides an interface to interact with playback. All configuration is stored within a .toml file which it must be loaded from. 

Music collections are stored in a sqlite3 database, which can be loaded from a directory/file.

Playback is handled by symphonia + cpal, which support most music file types.

## dango-gui:

TBD (egui current top candidate)

## dango-cli:

TBD