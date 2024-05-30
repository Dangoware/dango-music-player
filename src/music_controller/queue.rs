use crate::music_storage::library::Song;
use uuid::Uuid;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Index out of bounds! Index {index} is over len {len}")]
    OutOfBounds { index: usize, len: usize },
    #[error("The Queue is empty!")]
    EmptyQueue,
    #[error("There are no past played songs!")]
    EmptyPlayed,
    #[error("There is no item after this in the Queue")]
    NoNext,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    First,
    AddHere,
    NoState,
}

// TODO: move this to a different location to be used elsewhere
#[derive(Debug, Clone, Copy, PartialEq)]
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

impl QueueItem {
    fn from_song(song: Song, source: PlayerLocation) -> Self {
        QueueItem {
            item: song,
            state: QueueState::NoState,
            source,
            by_human: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub played: Vec<QueueItem>,
    pub loop_: bool,
    pub shuffle: Option<Vec<usize>>,
}

// TODO: HAndle the First QueueState[looping] and shuffle
impl Queue {
    fn has_addhere(&self) -> bool {
        for item in &self.items {
            if item.state == QueueState::AddHere {
                return true;
            }
        }
        false
    }

    #[allow(unused)]
    pub(super) fn dbg_items(&self) {
        dbg!(
            self.items
                .iter()
                .map(|item| (item.item.uuid, item.state))
                .collect::<Vec<(Uuid, QueueState)>>(),
            self.items.len()
        );
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    /// Inserts an item after the AddHere item
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

    /// Inserts an item after the currently playing item
    pub fn add_item_next(&mut self, item: Song, source: PlayerLocation) {
        use QueueState::*;
        let empty = self.items.is_empty();

        self.items.insert(
            if empty { 0 } else { 1 },
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

    pub fn add_multi(&mut self, items: Vec<Song>, source: PlayerLocation, by_human: bool) {
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

        let empty = self.items.is_empty();

        let len = items.len();
        for item in items.into_iter().rev() {
            self.items.insert(
                i + if empty { 0 } else { 1 },
                QueueItem {
                    item,
                    state: QueueState::NoState,
                    source,
                    by_human,
                },
            );
        }
        self.items[i + len - if empty { 1 } else { 0 }].state = QueueState::AddHere;
    }

    /// Add multiple Songs after the currently playing Song
    pub fn add_multi_next(&mut self, items: Vec<Song>, source: PlayerLocation) {
        use QueueState::*;
        let empty = self.items.is_empty();

        let add_here = (self.items.get(1).is_none()
            || !self.has_addhere() && self.items.get(1).is_some())
            || empty;

        let len = items.len();

        for item in items {
            self.items.insert(
                if empty { 0 } else { 1 },
                QueueItem {
                    item,
                    state: NoState,
                    source,
                    by_human: true,
                },
            )
        }

        if add_here {
            self.items[len - if empty { 1 } else { 0 }].state = QueueState::AddHere;
        }
    }

    pub fn remove_item(&mut self, remove_index: usize) -> Result<QueueItem, QueueError> {
        // dbg!(/*&remove_index, self.current_index(), &index,*/ &self.items[remove_index]);

        if remove_index < self.items.len() {
            // update the state of the next item to replace the item being removed
            if self.items.get(remove_index + 1).is_some() {
                self.items[remove_index + 1].state = self.items[remove_index].state;
            }
            Ok(self.items.remove(remove_index))
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn insert(
        &mut self,
        index: usize,
        new_item: Song,
        source: PlayerLocation,
        addhere: bool,
    ) -> Result<(), QueueError> {
        if self.items.get_mut(index).is_none()
            && index > 0
            && self.items.get_mut(index - 1).is_none()
        {
            return Err(QueueError::OutOfBounds {
                index,
                len: self.items.len(),
            });
        }
        if addhere {
            let mut new_item = QueueItem::from_song(new_item, source);
            for item in &mut self.items {
                if item.state == QueueState::AddHere {
                    item.state = QueueState::NoState
                }
            }
            new_item.state = QueueState::AddHere;
            self.items.insert(index, new_item);
        } else {
            let new_item = QueueItem::from_song(new_item, source);
            self.items.insert(index, new_item);
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn clear_except(&mut self, index: usize) -> Result<(), QueueError> {
        use QueueState::*;
        let empty = self.items.is_empty();

        if !empty && index < self.items.len() {
            let i = self.items[index].clone();
            self.items.retain(|item| *item == i);
            self.items[0].state = AddHere;
        } else if empty {
            return Err(QueueError::EmptyQueue);
        } else {
            return Err(QueueError::OutOfBounds {
                index,
                len: self.items.len(),
            });
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

    pub fn move_to(&mut self, index: usize) -> Result<(), QueueError> {
        use QueueState::*;

        let empty = self.items.is_empty();

        let index = if !empty {
            index
        } else {
            return Err(QueueError::EmptyQueue);
        };

        if !empty && dbg!(index < self.items.len()) {
            let to_item = self.items[index].clone();

            loop {
                let empty = self.items.is_empty();
                let item = self.items[0].item.to_owned();

                if item != to_item.item && !empty {
                    if self.items[0].state == AddHere && self.items.get(1).is_some() {
                        self.items[1].state = AddHere;
                    }
                    let item = self.items.remove(0);
                    self.played.push(item);

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

    pub fn move_item(&mut self, from: usize, to: usize) {
        let item = self.items[from].to_owned();
        if from != to {
            self.items.remove(from);
        }
        self.items.insert(to, item);
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<&QueueItem, QueueError> {
        if self.items.is_empty() {
            if self.loop_ {
                unimplemented!() // TODO: add function to loop the queue
            } else {
                return Err(QueueError::EmptyQueue);
            }
        }

        if self.items[0].state == QueueState::AddHere || !self.has_addhere() {
            self.items[0].state = QueueState::NoState;
            if self.items.get_mut(1).is_some() {
                self.items[1].state = QueueState::AddHere;
            }
        }
        let item = self.items.remove(0);
        self.played.push(item);

        if self.items.is_empty() {
            Err(QueueError::NoNext)
        } else {
            Ok(&self.items[0])
        }
    }

    pub fn prev(&mut self) -> Result<&QueueItem, QueueError> {
        if let Some(item) = self.played.pop() {
            if item.state == QueueState::First && self.loop_ {
                todo!()
            }
            self.items.insert(0, item);
            Ok(&self.items[0])
        } else {
            Err(QueueError::EmptyPlayed)
        }
    }

    pub fn now_playing(&self) -> Result<&QueueItem, QueueError> {
        if !self.items.is_empty() {
            Ok(&self.items[0])
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn check_played(&mut self) {
        while self.played.len() > 50 {
            self.played.remove(0);
        }
    }
}

#[cfg(test)]
mod test_super {
    #![allow(unused)]
    use crate::{
        config::tests::{new_config_lib, read_config_lib},
        music_storage::library,
    };

    use super::*;

    #[test]
    fn move_test() {
        let (_, library) = read_config_lib();
        let mut q = Queue::default();

        q.insert(0, library.library[2].to_owned(), PlayerLocation::File, true)
            .inspect_err(|e| println!("{e}"));
        q.insert(1, library.library[2].to_owned(), PlayerLocation::File, true)
            .inspect_err(|e| println!("{e}"));
        // q.next();
        // q.clear();
        q.dbg_items();
        dbg!(&q.played.len());

        // q.dbg_items();
    }
}
