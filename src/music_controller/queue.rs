use crate::music_storage::library::{MusicLibrary, Song, URI};
use std::{
    error::Error,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Index out of bounds! Index {0} is over len {1}")]
    OutOfBounds(usize, usize),
    #[error("The Queue is empty!")]
    EmptyQueue,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    First,
    AddHere,
    NoState,
}

// TODO: move this to a different location to be used elsewhere
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlayerLocation {
    Test,
    Library,
    Playlist(Uuid),
    File,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QueueItem {
    pub(super) item: Song,
    pub(super) state: QueueState,
    pub(super) source: PlayerLocation,
    pub(super) by_human: bool,
}
impl From<Song> for QueueItem {
    fn from(song: Song) -> Self {
        QueueItem {
            item: song,
            state: QueueState::NoState,
            source: PlayerLocation::Library,
            by_human: false,
        }
    }
}

#[derive(Debug)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub played: Vec<QueueItem>,
    pub loop_: bool,
    pub shuffle: bool,
}

impl Queue {
    fn has_addhere(&self) -> bool {
        for item in &self.items {
            if item.state == QueueState::AddHere {
                return true;
            }
        }
        false
    }

    fn dbg_items(&self) {
        dbg!(
            self.items
                .iter()
                .map(|item| item.item.clone())
                .collect::<Vec<Song>>(),
            self.items.len()
        );
    }

    pub fn new() -> Self {
        //TODO: Make the queue take settings from config/state if applicable
        Queue {
            items: Vec::new(),
            played: Vec::new(),
            loop_: false,
            shuffle: false,
        }
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    pub fn add_item(&mut self, item: Song, source: PlayerLocation, by_human: bool) {
        let mut i: usize = 0;

        self.items = self
            .items
            .iter()
            .enumerate()
            .map(|(j, item_)| {
                let mut item_ = item_.to_owned();
                // get the index of the current AddHere item and give it to i
                if item_.state == QueueState::AddHere {
                    i = j;
                    item_.state = QueueState::NoState;
                }
                item_
            })
            .collect::<Vec<QueueItem>>();

        self.items.insert(
            i + if self.items.is_empty() { 0 } else { 1 },
            QueueItem {
                item,
                state: QueueState::AddHere,
                source,
                by_human,
            },
        );
    }

    pub fn add_item_next(&mut self, item: Song, source: PlayerLocation) {
        use QueueState::*;
        let empty = self.items.is_empty();

        self.items.insert(
            (if empty { 0 } else { 1 }),
            QueueItem {
                item,
                state: if (self.items.get(1).is_none()
                    || !self.has_addhere() && self.items.get(1).is_some())
                    || empty
                {
                    AddHere
                } else {
                    NoState
                },
                source,
                by_human: true,
            },
        )
    }

    pub fn add_multi(&mut self, items: Vec<Song>, source: PlayerLocation, by_human: bool) {}

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
        } else {
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
            self.items.retain(|item| *item == i);
            self.items[0].state = AddHere;
        } else if empty {
            return Err("Queue is empty!".into());
        } else {
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

        let index = if !empty {
            index
        } else {
            return Err(QueueError::EmptyQueue);
        };

        if !empty && index < self.items.len() {
            let to_item = self.items[index].clone();

            loop {
                let empty = !self.items.is_empty();
                let item = self.items[0].item.to_owned();

                if item != to_item.item && !empty {
                    if self.items[0].state == AddHere && self.items.get(1).is_some() {
                        self.items[1].state = AddHere;
                    }
                    if let Err(e) = self.remove_item(0) {
                        dbg!(&e);
                        self.dbg_items();
                        return Err(e);
                    }
                // dbg!(&to_item.item, &self.items[ind].item);
                } else if empty {
                    return Err(QueueError::EmptyQueue);
                } else {
                    break;
                }
            }
        } else {
            return Err(QueueError::EmptyQueue);
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
    pub fn next(&mut self) -> Result<&QueueItem, Box<dyn Error>> {
        if self.items.is_empty() {
            if self.loop_ {
                return Err(QueueError::EmptyQueue.into()); // TODO: add function to loop the queue
            } else {
                return Err(QueueError::EmptyQueue.into());
            }
        }
        // TODO: add an algorithm to detect if the song should be skipped
        let item = self.items[0].clone();
        if self.items[0].state == QueueState::AddHere || !self.has_addhere() {
            self.items[1].state = QueueState::AddHere;
        }
        self.played.push(item);
        self.items.remove(0);

        Ok(&self.items[1])
    }

    pub fn prev() {}

    pub fn check_played(&mut self) {
        while self.played.len() > 50 {
            self.played.remove(0);
        }
    }
}
