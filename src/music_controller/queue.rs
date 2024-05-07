use uuid::Uuid;
use crate::{
    music_player::{Player, PlayerError},
    music_storage::library::{Album, MusicLibrary, URI}
};
use std::{
    error::Error,
    sync::{Arc, RwLock}
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Index out of bounds! Index {0} is over len {1}")]
    OutOfBounds(usize, usize),
    #[error("The Queue is empty!")]
    EmptyQueue

}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    First,
    AddHere,
    NoState,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum QueueItemType<'a> {
    Song(Uuid),
    ExternalSong(URI),
    Album{
        album: Album<'a>,
        shuffled: bool,
        order: Option<Vec<Uuid>>,
        // disc #, track #
        current: (i32, i32)
    },
    Playlist {
        uuid: Uuid,
        shuffled: bool,
        order: Option<Vec<Uuid>>,
        current: Uuid
    },
    None,
    Test
}

impl QueueItemType<'_> {
    fn get_uri(&self, lib: Arc<RwLock<MusicLibrary>>) -> Option<URI> {
        use QueueItemType::*;

        let lib = lib.read().unwrap();
        match self {
            Song(uuid) => {
                if let Some((song, _))  = lib.query_uuid(uuid) {
                    Some(song.location.clone())
                }else {
                    Option::None
                }
            },
            Album{album, shuffled, current: (disc, index), ..} => {
                if !shuffled {
                    Some(album.track(*disc as usize, *index as usize).unwrap().location.clone())
                }else {
                    todo!() //what to do for non shuffled album
                }
            },
            ExternalSong(uri) => { Some(uri.clone()) },
            _ => { Option::None }
        }
    }
}

// TODO: move this to a different location to be used elsewhere
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlayerLocation {
    Test,
    Library,
    Playlist(Uuid),
    File,
    Custom
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QueueItem<'a> {
    pub(super) item: QueueItemType<'a>,
    pub(super) state: QueueState,
    pub(super) source: PlayerLocation,
    pub(super) by_human: bool
}
impl QueueItem<'_> {
    fn new() -> Self {
        QueueItem {
            item: QueueItemType::None,
            state: QueueState::NoState,
            source: PlayerLocation::Library,
            by_human: false
        }
    }
}


#[derive(Debug)]
pub struct Queue<'a> {
    pub player: Player,
    pub name: String,
    pub items: Vec<QueueItem<'a>>,
    pub played: Vec<QueueItem<'a>>,
    pub loop_: bool
}

impl<'a> Queue<'a> {
    fn has_addhere(&self) -> bool {
        for item in &self.items {
            if item.state == QueueState::AddHere {
                return true
            }
        }
        false
    }

    fn dbg_items(&self) {
        dbg!(self.items.iter().map(|item| item.item.clone() ).collect::<Vec<QueueItemType>>(), self.items.len());
    }

    pub fn new() -> Result<Self, PlayerError> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            items: Vec::new(),
            played: Vec::new(),
            loop_: false,
            }
        )
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem<'a>>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    pub fn add_item(&mut self, item: QueueItemType<'a>, source: PlayerLocation, by_human: bool) {
        let mut i: usize = 0;

        self.items = self.items.iter().enumerate().map(|(j, item_)| {
            let mut item_ = item_.to_owned();
            // get the index of the current AddHere item and give it to i
            if item_.state == QueueState::AddHere {
                i = j;
                item_.state = QueueState::NoState;
            }
            item_
        }).collect::<Vec<QueueItem>>();

        self.items.insert(i + if self.items.is_empty() { 0 } else { 1 }, QueueItem {
            item,
            state: QueueState::AddHere,
            source,
            by_human
        });
    }

    pub fn add_item_next(&mut self, item: QueueItemType<'a>, source: PlayerLocation) {
        use QueueState::*;
        let empty = self.items.is_empty();

        self.items.insert(
            (if empty { 0 } else { 1 }),
            QueueItem {
                item,
                state: if (self.items.get(1).is_none() || (!self.has_addhere() && self.items.get(1).is_some()) || empty) { AddHere } else { NoState },
                source,
                by_human: true
            }
        )
    }

    pub fn add_multi(&mut self, items: Vec<QueueItemType>, source: PlayerLocation, by_human: bool) {

    }

    pub fn remove_item(&mut self, remove_index: usize) -> Result<(), QueueError> {

        // dbg!(/*&remove_index, self.current_index(), &index,*/ &self.items[remove_index]);

        if remove_index < self.items.len() {
            // update the state of the next item to replace the item being removed
            if self.items.get(remove_index + 1).is_some() {
                self.items[remove_index + 1].state = self.items[remove_index].state;
            }
            self.items[remove_index].state = QueueState::NoState;
            self.items.remove(remove_index);
            Ok(())
        }else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn clear_except(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let empty = self.items.is_empty();

        if !empty && index < self.items.len() {
            let i = self.items[index].clone();
            self.items.retain(|item| *item == i );
            self.items[0].state = AddHere;
        }else if empty {
            return Err("Queue is empty!".into());
        }else {
            return Err("index out of bounds!".into());
        }
        Ok(())
    }

    pub fn clear_played(&mut self) {
        self.played.clear();
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.played.clear();
    }


    // TODO: uh, fix this?
    fn move_to(&mut self, index: usize) -> Result<(), QueueError> {
        use QueueState::*;

        let empty = self.items.is_empty();
        let nothing_error = Err(QueueError::EmptyQueue);
        let index = if !empty { index } else { return nothing_error; };

        if !empty && index < self.items.len() {
            let position = self.player.position();
            if position.is_some_and(|dur| !dur.is_zero() ) {
                self.played.push(self.items[0].clone());
            }

            let to_item = self.items[index].clone();

            loop {
                let empty = !self.items.is_empty();
                let item = self.items[0].item.to_owned();

                if item != to_item.item && !empty {
                    if self.items[0].state == AddHere && self.items.get(1).is_some() {
                        self.items[1].state = AddHere;
                    }
                    if let Err(e) = self.remove_item(0) {
                        dbg!(&e); self.dbg_items(); return Err(e);
                    }
                // dbg!(&to_item.item, &self.items[ind].item);
                }else if empty {
                    return nothing_error;
                }else {
                    break;
                }
            }
        }else {
            return Err(QueueError::EmptyQueue.into());
        }
        Ok(())
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.items.swap(a, b)
    }

    pub fn move_item(&mut self, a: usize, b: usize) {
        let item = self.items[a].to_owned();
        if a != b {
            self.items.remove(a);
        }
        self.items.insert(b, item);
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self, lib: Arc<RwLock<MusicLibrary>>) -> Result<OutQueue, Box<dyn Error>> {



        if self.items.is_empty() {
            if self.loop_ {
                return Err(QueueError::EmptyQueue.into()); // TODO: add function to loop the queue
            }else {
                return Err(QueueError::EmptyQueue.into());
            }
        }
        // TODO: add an algorithm to detect if the song should be skipped
        let item = self.items[0].clone();
        let uri: URI = match &self.items[1].item {
            QueueItemType::Song(uuid) => {
                // TODO:  Refactor later for  multiple URIs
                match &lib.read().unwrap().query_uuid(uuid) {
                    Some(song) => song.0.location.clone(),
                    None => return Err("Uuid does not exist!".into()),
                }
            },
            QueueItemType::Album { album, current, ..} => {
                let (disc, track) = (current.0 as usize, current.1 as usize);
                match album.track(disc, track) {
                    Some(track) => track.location.clone(),
                    None => return Err(format!("Track in Album {} at disc {} track {} does not exist!", album.title(), disc, track).into())
                }
            },
            QueueItemType::Playlist { current, .. } => {
                // TODO:  Refactor later for  multiple URIs
                match &lib.read().unwrap().query_uuid(current) {
                    Some(song) => song.0.location.clone(),
                    None => return Err("Uuid does not exist!".into()),
                }
            },
            _ => todo!()
        };
        if !self.player.is_paused() {
            self.player.enqueue_next(&uri)?;
            self.player.play()?
        }
        if self.items[0].state == QueueState::AddHere || !self.has_addhere() {
            self.items[1].state = QueueState::AddHere;
        }
        self.played.push(item);
        self.items.remove(0);

        Ok(todo!())
    }

    pub fn prev() {}

    pub fn enqueue_item(&mut self, item: QueueItem, lib: Arc<RwLock<MusicLibrary>>) -> Result<(), Box<dyn Error>> {
        if let Some(uri) = item.item.get_uri(lib) {
            self.player.enqueue_next(&uri)?;
        }else {
            return Err("this item does not exist!".into());
        }
        Ok(())
    }
    pub fn check_played(&mut self) {
        while self.played.len() > 50 {
            self.played.remove(0);
        }
    }
}

pub struct OutQueue {

}

pub enum OutQueueItem {

}


#[test]
fn item_add_test() {
    let mut q = Queue::new().unwrap();

    for _ in 0..5 {
        // dbg!("tick!");
        q.add_item(QueueItemType::Song(Uuid::new_v4()), PlayerLocation::Library, true);
        // dbg!(&q.items, &q.items.len());
    }

    for _ in 0..1 {
    q.remove_item(0).inspect_err(|e| println!("{e:?}"));
    }
    for _ in 0..2 {
        q.items.push(QueueItem { item: QueueItemType::Test, state: QueueState::NoState, source: PlayerLocation::Library, by_human: false });
    }
    dbg!(5);

    q.add_item_next(QueueItemType::Test, PlayerLocation::Test);
    dbg!(6);

    dbg!(&q.items, &q.items.len());
}

#[test]
fn test_() {
    let mut q = Queue::new().unwrap();
    for _ in 0..400 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::NoState, source: PlayerLocation::File, by_human: false });
    }
    for _ in 0..50000 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), PlayerLocation::Library, true);
    }
    // q.add_item_next(QueueItemType::Test, PlayerLocation::File);

    // dbg!(&q.items, &q.items.len());

}

#[test]
fn move_test() {
    let mut q = Queue::new().unwrap();

    for _ in 0..5 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), PlayerLocation::Library, true);
    }
    // q.add_item(QueueItemType::Test, QueueSource::Library, true).unwrap();
    dbg!(&q.items, &q.items.len());

    q.move_to(3).inspect_err(|e| {dbg!(e);});
    dbg!(&q.items, &q.items.len());
    // q.dbg_items();
}
