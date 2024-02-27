use uuid::Uuid;

use crate::{music_player::Player, music_storage::library::{Album, Song, URI}};
use std::{error::Error, ops::Add, path::Path};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    Current,
    AddHere,
    None,
}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum QueueItemType<'a> {
    Song(Uuid),
    ExternalSong(URI),
    Album{
        album: Album<'a>,
        shuffled: bool,
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum QueueSource {
    Library,
    Playlist(Uuid),
    Search,
    Queue,
    File,
}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct QueueItem<'a> {
    item: QueueItemType<'a>,
    state: QueueState,
    source: QueueSource
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
        while e >= 51 {
            self.items.remove(0);
            e = self.items.iter().filter(|song| song.state == QueueState::Played ).collect::<Vec<&QueueItem>>().len();
            i+=1;
        }
        e as i16 - 1
    }

    pub fn add_item(&mut self, item: QueueItemType<'a>, source: QueueSource) -> Result<(), Box<dyn Error>> {
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
                source
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
            }
            self.items[ind].state = None;
            self.items.remove(ind);

            Ok(())
        }else {
            Err("No Songs to remove!".into())
        }
    }
}


#[test]
fn item_add_test() {
    let mut q = Queue::new().unwrap();
    for _ in 0..5 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::Queue });
    }
    for _ in 0..3 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library).unwrap();
    }
    dbg!(&q.items, &q.items.len());

    for _ in 0..1 {
    q.remove_item(0).inspect_err(|e| println!("{e:?}"));
    dbg!(&q.items.len());
    }

    dbg!(&q.items, &q.items.len());
}