use uuid::Uuid;

use crate::{music_player::Player, music_storage::library::{Album, Song}};
use std::{error::Error, path::Path};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    Current,
    AddHere,
    None,
}
#[derive(Debug, Clone)]
pub enum QueueItemType<'a> {
    Song(Uuid),
    Album(Album<'a>)
}
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct QueueItem<'a> {
    item: QueueItemType<'a>,
    state: QueueState
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

    pub fn current_index(&self) -> i16 {
        let e  = self.items.iter().filter(|song| song.state == QueueState::Played ).collect::<Vec<&QueueItem>>().len();
        dbg!(&e);
                e as i16 - 1
    }

    pub fn add_item(&mut self, item: QueueItemType<'a>) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let ind = self.current_index();
        let mut i: i16 = 1;
        self.items = self.items.iter().enumerate().map(|(j, item_)| {
            let mut item_ = item_.to_owned();
            if item_.state == AddHere {
                i = j as i16 + 2;
                item_.state = None;
            }
            if item_.state == Current {
                i = j as i16 + 2;
            }
            item_
        }).collect::<Vec<QueueItem>>();
        let pos = (ind + i) as usize;
        dbg!(&pos);
        self.items.insert(
            pos,
            QueueItem {
                item: item.clone(),
                state: if pos == self.items.len() && i == 1 {
                    Current
                }else {
                    AddHere
                }
            }
        );
        Ok(())
    }
}


#[test]
fn itemaddtest() {
    let mut q = Queue::new().unwrap();
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
    q.add_item(QueueItemType::Song(Uuid::new_v4())).unwrap();
    dbg!(&q.items);
}