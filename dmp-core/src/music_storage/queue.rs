use std::fmt::Debug;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    First,
    AddHere,
    NoState,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QueueItem {
    pub item: QueueItemType,
    pub state: QueueState,
    pub by_human: bool,
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

impl From<QueueItemType> for QueueItem {
    fn from(value: QueueItemType) -> Self {
        QueueItem {
            item: value,
            state: QueueState::NoState,
            by_human: false,
            location: todo!(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub played: Vec<QueueItem>,
    pub looping: bool,
    pub shuffle: Option<Vec<usize>>,
    pub pull_location: Option<PlayerLocation>,
}

// TODO: HAndle the First QueueState[looping] and shuffle
impl Queue {
    fn has_addhere(&self) -> bool {
        todo!()
    }

    #[allow(unused)]
    pub(crate) fn dbg_items(&self) {
        dbg!(
            self.items
                .iter()
                .map(|item| (&item.item, item.state))
                .collect::<Vec<(&QueueItemType, QueueState)>>(),
            self.items.len()
        );
    }

    pub fn new(loop_: bool, shuffle: Option<Vec<usize>>) -> Self {
        Queue {
            items: Vec::new(),
            played: Vec::new(),
            looping: loop_,
            shuffle,
        }
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    /// Inserts an item after the AddHere item
    pub fn add_item(&mut self, item: QueueItemType, by_human: bool) {
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

        if !empty {
            self.items
                .get_mut(i)
                .expect("There should be an item at index {i}")
                .state = QueueState::NoState;
        }

        if by_human {
            self.items.insert(
                i + if empty { 0 } else { 1 },
                QueueItem {
                    item,
                    state: QueueState::AddHere,
                    by_human,
                    location: PlayerLocation::Test,
                },
            );
        } else {
            self.items.push(QueueItem {
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
                by_human: true,
            },
        )
    }

    pub fn add_multi(&mut self, items: Vec<QueueItemType>, by_human: bool) {
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
        if !empty {
            self.items
                .get_mut(i)
                .expect("There should be an item at index {i}")
                .state = QueueState::NoState;
        }

        let len = items.len();
        for item in items.into_iter().rev() {
            if by_human {
                self.items.insert(
                    i + if empty { 0 } else { 1 },
                    QueueItem {
                        item,
                        state: QueueState::NoState,
                        by_human, // true
                    },
                );
            } else {
                self.items.push(QueueItem {
                    item,
                    state: QueueState::NoState,
                    by_human, // false
                });
            }
        }
        self.items[i + len - if empty { 1 } else { 0 }].state = QueueState::AddHere;
    }

    /// Add multiple Items after the currently playing Item
    pub fn add_multiple_next(&mut self, items: Vec<QueueItemType>) {
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
        new_item: QueueItemType,
        addhere: bool,
    ) -> Result<(), QueueError> {
        todo!()
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
        if self.items.is_empty() {
            return Err(QueueError::EmptyQueue);
        }
        for _ in 0..index {
            self.next()?;
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
            if self.looping {
                unimplemented!() // TODO: add function to loop the queue
            } else {
                return Err(QueueError::EmptyQueue);
            }
        }

        if self.items[0].state == QueueState::AddHere || !self.has_addhere() {
            if let QueueItemType::Album { .. } = self.items[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }

            self.items[0].state = QueueState::NoState;
            if self.items.get(1).is_some() {
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
            if item.state == QueueState::First && self.looping {
                todo!()
            }

            if let QueueItemType::Album { .. } = self.items[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }
            if let QueueItemType::Album { .. } = item.item {
                unimplemented!(); // TODO: Handle Multi items here?
            }

            self.items.insert(0, item);
            Ok(&self.items[0])
        } else {
            Err(QueueError::EmptyPlayed)
        }
    }

    pub fn current(&self) -> Result<&QueueItem, QueueError> {
        if !self.items.is_empty() {
            if let QueueItemType::Album { .. } = self.items[0].item {
                unimplemented!(); // TODO: Handle Multi items here?
            }
            Ok(&self.items[0])
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

use thiserror::Error;

use crate::music_controller::controller::PlayerLocation;

use super::library::{Album, Song};

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
