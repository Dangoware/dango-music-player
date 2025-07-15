use std::{fmt::Debug, path::PathBuf};
use thiserror::Error;
use uuid::Uuid;

use crate::music_controller::controller::PlayerLocation;

use super::library::{Album, Song};

#[derive(Debug, Clone, Default)]
pub struct Queue {
    pub queue: Vec<QueueItem>,
    pub up_next_visible: Vec<QueueItem>,
    pub up_next_invisible: Vec<UpNextSong>,
    pub played: Vec<QueueItem>,
    pub looping: Loop,
    pub shuffle: Shuffle,
    pub pull_location: Option<PlayerLocation>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QueueItem {
    pub item: QueueItemType,
    pub location: PlayerLocation,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum QueueItemType {
    Song(Song),
    Album {
        album: Album,
        current: usize,
        shuffled: bool,
        looping: bool,
    },
}

#[derive(Debug, Clone)]
enum UpNextSong {
    Library(Uuid),
    File(PathBuf),
}

#[derive(Debug, Clone, Default)]
enum Shuffle {
    #[default]
    NoShuffle,
    ShuffleAllSongs,
    ShuffleInCategory,
    ShuffleCategories,
    ShuffleInCategoriesAndCategories,
    // Future feature: Add Dango Shuffle Option
}

#[derive(Debug, Clone, Default)]
pub enum Loop {
    #[default]
    NoLoop,
    LoopCategory,
    LoopSong,
    LoopAlbum,
    ABLoop,
    PlayOnce,
}

// TODO: HAndle the First QueueState[looping] and shuffle
impl Queue {
    pub fn set_items(&mut self, tracks: Vec<QueueItem>) {
        let mut tracks = tracks;
        self.queue.clear();
        self.queue.append(&mut tracks);
    }

    /// Inserts an item after the AddHere item
    pub fn add_item(&mut self, item: QueueItemType, by_human: bool) {
        let mut i: usize = 0;

        self.queue = self
            .queue
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
        let empty = self.queue.is_empty();

        if !empty {
            self.queue
                .get_mut(i)
                .expect("There should be an item at index {i}")
                .state = QueueState::NoState;
        }

        if by_human {
            self.queue.insert(
                i + if empty { 0 } else { 1 },
                QueueItem {
                    item,
                    state: QueueState::AddHere,
                    by_human,
                    location: PlayerLocation::Test,
                },
            );
        } else {
            self.queue.push(QueueItem {
                item,
                state: QueueState::NoState,
                by_human,
                location: PlayerLocation::Test,
            });
        }
    }

    /// Inserts an item after the currently playing item
    pub fn add_item_next(&mut self, item: QueueItemType) {
        use QueueState::*;
        let empty = self.queue.is_empty();

        self.queue.insert(
            if empty { 0 } else { 1 },
            QueueItem {
                item,
                state: if (self.queue.get(1).is_none()
                    || !self.has_addhere() && self.queue.get(1).is_some())
                    || empty
                {
                    AddHere
                } else {
                    NoState
                },
                by_human: true,
            },
        )
    }

    pub fn add_multi(&mut self, items: Vec<QueueItemType>, by_human: bool) {
        let mut i: usize = 0;

        self.queue = self
            .queue
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

        let empty = self.queue.is_empty();
        if !empty {
            self.queue
                .get_mut(i)
                .expect("There should be an item at index {i}")
                .state = QueueState::NoState;
        }

        let len = items.len();
        for item in items.into_iter().rev() {
            if by_human {
                self.queue.insert(
                    i + if empty { 0 } else { 1 },
                    QueueItem {
                        item,
                        state: QueueState::NoState,
                        by_human, // true
                    },
                );
            } else {
                self.queue.push(QueueItem {
                    item,
                    state: QueueState::NoState,
                    by_human, // false
                });
            }
        }
        self.queue[i + len - if empty { 1 } else { 0 }].state = QueueState::AddHere;
    }

    /// Add multiple Items after the currently playing Item
    pub fn add_multiple_next(&mut self, items: Vec<QueueItemType>) {
        use QueueState::*;
        let empty = self.queue.is_empty();

        let add_here = (self.queue.get(1).is_none()
            || !self.has_addhere() && self.queue.get(1).is_some())
            || empty;

        let len = items.len();

        for item in items {
            self.queue.insert(
                if empty { 0 } else { 1 },
                QueueItem {
                    item,
                    state: NoState,
                    by_human: true,
                },
            )
        }

        if add_here {
            self.queue[len - if empty { 1 } else { 0 }].state = QueueState::AddHere;
        }
    }

    pub fn remove_item(&mut self, remove_index: usize) -> Result<QueueItem, QueueError> {
        // dbg!(/*&remove_index, self.current_index(), &index,*/ &self.items[remove_index]);

        if remove_index < self.queue.len() {
            // update the state of the next item to replace the item being removed
            if self.queue.get(remove_index + 1).is_some() {
                self.queue[remove_index + 1].state = self.queue[remove_index].state;
            }
            Ok(self.queue.remove(remove_index))
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn insert(
        &mut self,
        index: usize,
        new_item: QueueItemType,
        addhere: bool,
    ) -> Result<(), QueueError> {
        todo!()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn clear_except(&mut self, index: usize) -> Result<(), QueueError> {
        use QueueState::*;
        let empty = self.queue.is_empty();

        if !empty && index < self.queue.len() {
            let i = self.queue[index].clone();
            self.queue.retain(|item| *item == i);
            self.queue[0].state = AddHere;
        } else if empty {
            return Err(QueueError::EmptyQueue);
        } else {
            return Err(QueueError::OutOfBounds {
                index,
                len: self.queue.len(),
            });
        }
        Ok(())
    }

    pub fn clear_played(&mut self) {
        self.played.clear();
    }

    pub fn clear_all(&mut self) {
        self.queue.clear();
        self.played.clear();
    }

    pub fn move_to(&mut self, index: usize) -> Result<(), QueueError> {
        if self.queue.is_empty() {
            return Err(QueueError::EmptyQueue);
        }
        for _ in 0..index {
            self.next()?;
        }

        Ok(())
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.queue.swap(a, b)
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        let item = self.queue[from].to_owned();
        if from != to {
            self.queue.remove(from);
        }
        self.queue.insert(to, item);
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<&QueueItem, QueueError> {
        if self.queue.is_empty() {
            if self.looping {
                unimplemented!() // TODO: add function to loop the queue
            } else {
                return Err(QueueError::EmptyQueue);
            }
        }

        if self.queue[0].state == QueueState::AddHere || !self.has_addhere() {
            if let QueueItemType::Album { .. } = self.queue[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }

            self.queue[0].state = QueueState::NoState;
            if self.queue.get(1).is_some() {
                self.queue[1].state = QueueState::AddHere;
            }
        }
        let item = self.queue.remove(0);
        self.played.push(item);

        if self.queue.is_empty() {
            Err(QueueError::NoNext)
        } else {
            Ok(&self.queue[0])
        }
    }

    pub fn prev(&mut self) -> Result<&QueueItem, QueueError> {
        if let Some(item) = self.played.pop() {
            if item.state == QueueState::First && self.looping {
                todo!()
            }

            if let QueueItemType::Album { .. } = self.queue[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }
            if let QueueItemType::Album { .. } = item.item {
                unimplemented!(); // TODO: Handle Multi items here?
            }

            self.queue.insert(0, item);
            Ok(&self.queue[0])
        } else {
            Err(QueueError::EmptyPlayed)
        }
    }

    pub fn current(&self) -> Result<&QueueItem, QueueError> {
        if !self.queue.is_empty() {
            if let QueueItemType::Album { .. } = self.queue[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }
            Ok(&self.queue[0])
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn check_played(&mut self, limit: usize) {
        while self.played.len() > limit {
            self.played.remove(0);
        }
    }
}

#[derive(Error, Debug, PartialEq, Eq, PartialOrd, Clone)]
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
