use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use ts_rs::TS;
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
    pub up_next_limit: usize,
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
    Album { album: Album, current: usize },
}

#[derive(Debug, PartialEq, Clone)]
enum UpNextSong {
    Library(Uuid),
    File(PathBuf),
}

#[cfg_attr(feature = "ts", derive(TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Shuffle {
    #[default]
    NoShuffle,
    ShuffleAllSongs,
    ShuffleInCategory,
    ShuffleCategories,
    ShuffleInCategoriesAndCategories,
    // Future feature: Add Dango Shuffle Option
}

#[cfg_attr(feature = "ts", derive(TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Loop {
    #[default]
    NoLoop,
    LoopCategory,
    LoopSong,
    LoopAlbum,
    ABLoop,
    PlayOnce,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueNext {
    /// The current item in the queue
    pub item: QueueItem,
    /// The item pulled from the invisible up next section to be parsed and appended to the visible section
    pub up_next_item: Option<UpNextSong>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueMove {
    /// The current item in the queue
    pub item: QueueItem,
    /// The items pulled from the invisible up next section to be parsed and appended to the visible section
    pub up_next_items: Vec<UpNextSong>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            up_next_limit: 50,
            ..Default::default()
        }
    }

    /// Sets all items in the queue and next up section
    pub fn set_items(&mut self, tracks: Vec<QueueItem>) {
        let mut tracks = tracks;
        self.queue.clear();
        self.queue.append(&mut tracks);
    }

    /// Append an item to the queue
    pub fn add_item(&mut self, item: QueueItem) {
        self.queue.push(item);
    }

    /// Inserts an item in the queue after the now playing item
    pub fn add_after_np(&mut self, item: QueueItem) {
        if self.queue.is_empty() {
            self.queue.push(item);
        } else {
            self.queue.insert(1, item);
        }
    }

    /// Append multiple items to the queue
    pub fn add_multiple(&mut self, items: Vec<QueueItem>) {
        for item in items {
            self.queue.push(item);
        }
    }

    /// Insert multiple items to the queue after the now playing Item
    pub fn add_multiple_after_np(&mut self, mut items: Vec<QueueItem>) {
        if self.queue.is_empty() {
            for item in items {
                self.queue.push(item);
            }
        } else {
            items.reverse();
            for item in items {
                self.queue.insert(1, item);
            }
        }
    }

    /// Remove an item from the queue at the selected index
    pub fn remove_queue(&mut self, index: usize) -> Result<QueueItem, QueueError> {
        let len = self.queue.len();
        if len > 0 && len > index {
            Ok(self.queue.remove(index))
        } else {
            Err(QueueError::OutOfBounds { index, len })
        }
    }

    /// Remove an item from the next up section at the selected index
    pub fn remove_next_up(&mut self, index: usize) -> Result<QueueItem, QueueError> {
        let len = self.up_next_visible.len();
        if len > 0 && len > index {
            Ok(self.up_next_visible.remove(index))
        } else {
            Err(QueueError::OutOfBounds { index, len })
        }
    }

    /// Insert an item into the queue at the selected index
    pub fn insert_queue(&mut self, index: usize, item: QueueItem) -> Result<(), QueueError> {
        let len = self.queue.len();
        if len > index || len == 0 && index == 0 {
            self.queue.insert(index, item);
            Ok(())
        } else {
            Err(QueueError::OutOfBounds { index, len })
        }
    }

    /// Clear all sections of the queue except for the played stack
    pub fn clear(&mut self) {
        self.queue.clear();
        self.up_next_visible.clear();
        self.up_next_invisible.clear();
    }

    /// Clear all sections of the queue except for the selected index in the queue and the played stack
    pub fn clear_except_queue(&mut self, index: usize) -> Result<(), QueueError> {
        let Some(item) = self.queue.get(index).map(|i| i.clone()) else {
            return Err(QueueError::OutOfBounds {
                index,
                len: self.queue.len(),
            });
        };

        self.queue.retain(|item_| &item == item_);
        self.up_next_visible.clear();
        self.up_next_invisible.clear();
        Ok(())
    }

    /// Clear all sections of the queue except for the selected index in the next up section and the played stack
    pub fn clear_except_next_up(&mut self, index: usize) -> Result<(), QueueError> {
        let Some(item) = self.up_next_visible.get(index).map(|i| i.clone()) else {
            return Err(QueueError::OutOfBounds {
                index,
                len: self.up_next_visible.len(),
            });
        };

        self.up_next_visible.retain(|item_| &item == item_);
        self.queue.clear();
        self.up_next_invisible.clear();
        Ok(())
    }

    /// Clear all items from the played stack
    pub fn clear_played(&mut self) {
        self.played.clear();
    }

    pub fn clear_all(&mut self) {
        self.played.clear();
        self.queue.clear();
        self.up_next_visible.clear();
        self.up_next_invisible.clear();
    }

    pub fn move_to(&mut self, index: usize) -> Result<QueueMove, QueueError> {
        let queue_len = self.queue.len();
        let up_next_len = self.up_next_visible.len();
        let prev = self.queue.remove(0);
        self.played.push(prev);
        let mut up_next_items = vec![];

        if index < queue_len {
            if let Some(_) = self.queue.get(index) {
                for _ in 0..index {
                    if let Ok(QueueNext {
                        up_next_item: Some(next),
                        ..
                    }) = self.skip()
                    {
                        up_next_items.push(next);
                    }
                }
            } else {
                unreachable!("Queue couldn't get item to move to");
            }
        } else if index < self.up_next_visible.len() {
            if let Some(_) = self.queue.get(index) {
                for _ in 0..(index - queue_len) {
                    for _ in 0..index {
                        if let Ok(QueueNext {
                            up_next_item: Some(next),
                            ..
                        }) = self.skip()
                        {
                            up_next_items.push(next);
                        }
                    }
                }
            } else {
                unreachable!("Queue couldn't get item to move to");
            }
        } else {
            return Err(QueueError::OutOfBounds {
                index,
                len: queue_len + up_next_len,
            });
        }
        Ok(QueueMove {
            item: self.queue[0].clone(),
            up_next_items,
        })
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<QueueNext, QueueError> {
        if !self.queue.is_empty() {
            'album: {
                match self.queue.get(0) {
                    Some(item) => {
                        if let QueueItemType::Album {
                            ref album,
                            ref current,
                        } = item.item
                        {
                            let new_index = if current < &album.len() {
                                *current + 1
                            } else if *current >= album.len()
                                && let Loop::LoopAlbum = self.looping
                            {
                                0
                            } else {
                                break 'album;
                            };
                            let next = QueueNext {
                                item: QueueItem {
                                    item: QueueItemType::Album {
                                        album: album.clone(),
                                        current: new_index,
                                    },
                                    location: item.location.clone(),
                                },
                                up_next_item: None,
                            };

                            return Ok(next);
                        }
                    }
                    None => unreachable!(),
                }
            }

            let prev = self.queue.remove(0);
            self.played.push(prev);
            if self.queue.is_empty() {
                Ok(QueueNext {
                    item: self.queue[0].clone(),
                    up_next_item: if self.up_next_invisible.get(0).is_some()
                        && self.up_next_visible.len() < self.up_next_limit
                    {
                        Some(self.up_next_invisible.remove(0))
                    } else {
                        None
                    },
                })
            } else if self.queue.is_empty() && !self.up_next_visible.is_empty() {
                let new = self.up_next_visible.remove(0);
                self.queue.push(new);
                Ok(QueueNext {
                    item: self.queue[0].clone(),
                    up_next_item: if self.up_next_invisible.get(0).is_some()
                        && self.up_next_visible.len() < self.up_next_limit
                    {
                        Some(self.up_next_invisible.remove(0))
                    } else {
                        None
                    },
                })
            } else {
                Err(QueueError::NoNext)
            }
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn prev(&mut self) -> Result<&QueueItem, QueueError> {
        let Some(prev) = self.played.pop() else {
            return Err(QueueError::EmptyPlayed);
        };

        self.queue.insert(0, prev);
        Ok(&self.queue[0])
    }

    pub fn skip(&mut self) -> Result<QueueNext, QueueError> {
        if !self.queue.is_empty() {
            'album: {
                match self.queue.get(0) {
                    Some(item) => {
                        if let QueueItemType::Album {
                            ref album,
                            ref current,
                        } = item.item
                        {
                            let new_index = if current < &album.len() {
                                *current + 1
                            } else if *current >= album.len()
                                && let Loop::LoopAlbum = self.looping
                            {
                                0
                            } else {
                                break 'album;
                            };
                            let next = QueueNext {
                                item: QueueItem {
                                    item: QueueItemType::Album {
                                        album: album.clone(),
                                        current: new_index,
                                    },
                                    location: item.location.clone(),
                                },
                                up_next_item: None,
                            };

                            return Ok(next);
                        }
                    }
                    None => unreachable!(),
                }
            }

            // This is the only line changed from the `Queue::next()` function
            _ = self.queue.remove(0);
            if self.queue.is_empty() {
                Ok(QueueNext {
                    item: self.queue[0].clone(),
                    up_next_item: if self.up_next_invisible.get(0).is_some()
                        && self.up_next_visible.len() < self.up_next_limit
                    {
                        Some(self.up_next_invisible.remove(0))
                    } else {
                        None
                    },
                })
            } else if self.queue.is_empty() && !self.up_next_visible.is_empty() {
                let new = self.up_next_visible.remove(0);
                self.queue.push(new);
                Ok(QueueNext {
                    item: self.queue[0].clone(),
                    up_next_item: if self.up_next_invisible.get(0).is_some()
                        && self.up_next_visible.len() < self.up_next_limit
                    {
                        Some(self.up_next_invisible.remove(0))
                    } else {
                        None
                    },
                })
            } else {
                Err(QueueError::NoNext)
            }
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn current(&self) -> Result<&QueueItem, QueueError> {
        if let Some(item) = self.queue.get(0) {
            Ok(item)
        } else {
            Err(QueueError::EmptyQueue)
        }
    }

    pub fn check_played(&mut self, limit: usize) {
        let len = self.played.len();
        if len > limit {
            for _ in 0..(len - limit) {
                self.played.remove(0);
            }
        }
    }

    pub fn get_next_up_to_limit(&mut self) -> Vec<UpNextSong> {
        let mut out = vec![];

        for _ in 0..self.up_next_limit {
            if self.up_next_invisible.get(0).is_some() {
                out.push(self.up_next_invisible.remove(0));
            } else {
                break;
            }
        }
        out
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
