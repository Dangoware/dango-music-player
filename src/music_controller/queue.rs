use font::opentype::tables::font_variations::InstanceFlags;
use uuid::Uuid;

use crate::{music_player::Player, music_storage::library::{Album, Song, URI}};
use std::{error::Error, path::Path};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    Current,
    AddHere,
    None,
}
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum QueueItemType<'a> {
    Song(Uuid),
    ExternalSong(URI),
    Album{
        album: Album<'a>,
        shuffled: bool,
    },
    None
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum QueueSource {
    Library,
    Playlist(Uuid),
    Search,
    Queue,
    File,
}
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QueueItem<'a> {
    item: QueueItemType<'a>,
    state: QueueState,
    source: QueueSource,
    by_human: bool
}
impl QueueItem<'_> {
    fn new() -> Self {
        QueueItem {
            item: QueueItemType::None,
            state: QueueState::None,
            source: QueueSource::Library,
            by_human: false
        }
    }
}


#[derive(Debug)]
pub struct Queue<'a> {
    pub player: Player,
    pub name: String,
    pub items: Vec<QueueItem<'a>>,
}

impl<'a> Queue<'a> {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            items: Vec::new()
            }
        )
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem<'a>>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    pub fn current_index(&mut self) -> i16 {
        let mut i = 1;
        let mut e  = self.items.iter().filter(|song| song.state == QueueState::Played ).collect::<Vec<&QueueItem>>().len();
        // TODO: make the max number of past songs modular
        while e > 50 {
            self.items.remove(0);
            e = self.items.iter().filter(|song| song.state == QueueState::Played ).collect::<Vec<&QueueItem>>().len();
            i+=1;
        }
        e as i16 - 1
    }

    pub fn add_item(&mut self, item: QueueItemType<'a>, source: QueueSource, by_human: bool) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let ind = self.current_index();
        let mut i: i16 = 1;
             self.items = self.items.iter().enumerate().map(|(j, item_)| {
            let mut item_ = item_.to_owned();
            if item_.state == AddHere {
                i = j as i16 + 1 - ind;
                item_.state = None;
            }
            if item_.state == Current {
                i = j as i16 + 1 - ind;
            }
            item_
        }).collect::<Vec<QueueItem>>();
        let pos = (ind + i) as usize;
        // dbg!(&pos, &i, &ind);
        self.items.insert(
            pos,
            QueueItem {
                item: item.clone(),
                state: if pos == self.items.len() && i == 1 {
                    Current
                }else {
                    AddHere
                },
                source,
                by_human
            }
        );
        Ok(())
    }

    pub fn remove_item(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let ind = (self.current_index() + index as i16 + 1) as usize;

        if ind < self.items.len() {
            // update the state of the next item to replace the item being removed
            if self.items.get(ind + 1).is_some() {
                self.items[ind + 1].state = self.items[ind].state;
            }else if self.items[ind].state != Current {
                self.items[ind - 1].state = self.items[ind].state;
            }
            self.items[ind].state = None;
            self.items.remove(ind);
            Ok(())
        }else {
            Err("No Songs to remove!".into())
        }
    }

    pub fn clear(&mut self) {
        self.items.retain(|item| item.state == QueueState::Played );
    }

    pub fn clear_except(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        let mut index = index;
        let ind = self.current_index();

        if ind != -1 {
            index += ind as usize;
        }else {
            index -=1
        }

        if !self.is_empty() && index < self.items.len() {
            let i = self.items[index].clone();
            self.items.retain(|item| item.state == QueueState::Played || *item == i );
            self.items[(ind+1) as usize].state = QueueState::Current
        }else {
            return Err("index out of bounds!".into());
        }
        Ok(())
    }

    pub fn clear_played(&mut self) {
        self.items.retain(|item| item.state != QueueState::Played );
    }

    pub fn clear_all(&mut self) {
        self.items.clear()
    }

    fn is_empty(&self) -> bool {
        self.items.iter().filter(|item| item.state != QueueState::Played).collect::<Vec<_>>().len() == 0
    }
}


#[test]
fn item_add_test() {
    let mut q = Queue::new().unwrap();
    for _ in 0..5 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::Queue, by_human: false });
    }
    q.clear();
    for _ in 0..5 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    // q.clear_played();
    for _ in 0..3 {
    q.remove_item(0).inspect_err(|e| println!("{e:?}"));
    }
    q.clear_except(4).inspect_err(|e| println!("{e:?}"));

    dbg!(&q.items, &q.items.len());
}