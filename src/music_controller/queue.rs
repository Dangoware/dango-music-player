use font::opentype::tables::font_variations::InstanceFlags;
use uuid::Uuid;

use crate::{music_player::Player, music_storage::library::{Album, MusicLibrary, Song, URI}};
use std::{error::Error, ops::Add, path::Path, sync::{Arc, RwLock}, thread::sleep, time::Duration};

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
        // disc #, track #
        current: (i32, i32)
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
            Album{album, shuffled, current: (disc, index)} => {
                if !shuffled {
                    Some(album.track(*disc as usize, *index as usize).unwrap().location.clone())
                }else {
                    todo!()
                }
            },
            ExternalSong(uri) => { Some(uri.clone()) },
            _ => { Option::None }
        }
    }
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

    fn contains_state(&self, state: QueueState) -> bool {
        !self.items.iter().filter(|item| item.state == state ).collect::<Vec<_>>().is_empty()
    }

    fn is_empty(&self) -> bool {
        self.items.iter().filter(|item| item.state != QueueState::Played).collect::<Vec<_>>().is_empty()
    }

    pub fn set_items(&mut self, tracks: Vec<QueueItem<'a>>) {
        let mut tracks = tracks;
        self.items.clear();
        self.items.append(&mut tracks);
    }

    pub fn add_item(&mut self, item: QueueItemType<'a>, source: QueueSource, by_human: bool) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let ind = self.current_index();
        let mut i: i16 = 1;
        self.items = self.items.iter().enumerate().map(|(j, item_)| {
            let mut item_ = item_.to_owned();
            // get the index of the current AddHere item and give it to i
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

    pub fn add_item_next(&mut self, item: QueueItemType<'a>, source: QueueSource) {
        use QueueState::*;
        let ind = self.current_index();
        let empty = self.is_empty();

        self.items.insert(
            // index would go out of bounds if empty ( current index = -1 )
            if empty {
                (ind + 1) as usize
            }else {
                (ind + 2) as usize
            },
            QueueItem {
                item,
                state: if empty {
                    Current
                }else if self.items.get((ind + 1) as usize).is_none() || (!self.contains_state(AddHere) && self.items.get((ind + 1) as usize).is_some()) {
                    AddHere
                }else {
                    None
                },
                source,
                by_human: true
            }
        )
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
        let empty = self.is_empty();

        if !empty {
            index += ind as usize;
        }else {
            index -=1
        }

        if !empty && index < self.items.len() {
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

    fn move_to(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        let mut index = index;
        let empty = self.is_empty();
        let ind = self.current_index();

        if !empty {
            index += ind as usize;
        }else {
            return Err("Nothing in the queue to move to!".into());
        }

        dbg!(1);
        if !empty && index < self.items.len() -1 {
            // TODO: make this check for player position
            let pos = self.player.position();
            if pos.is_some_and(|dur| !dur.is_zero() ) {
                self.items[ind as usize].state = QueueState::Played
            }
            dbg!(2);

            let to_item = self.items[index].clone();
            let new_ind = self.current_index() as usize;
            dbg!(3);

            // dbg!(&self.items, &new_ind, &to_item.item, &self.items[new_ind + 1].item, &self.items.len());
            loop {
            dbg!(4);

                if self.items[new_ind + 1].item != to_item.item {
                    self.remove_item(0);
                dbg!(&self.items, &new_ind, &to_item.item, &self.items[new_ind + 1].item, &self.items.len());
                sleep(Duration::from_millis(1000));
                }else {
                    break;
                }
            }
        }else {
            return Err("index out of bounds!".into());
        }
        Ok(())
    }

    pub fn swap(&mut self, index1: usize, index2: usize) {}

    pub fn move_item(&mut self, item: usize, to_index: usize) {}

    pub fn next() {}

    pub fn prev() {}

    pub fn enqueue_item(&mut self, item: QueueItem, lib: Arc<RwLock<MusicLibrary>>) -> Result<(), Box<dyn Error>> {
        use QueueItemType::*;

        if let Some(uri) = item.item.get_uri(lib) {
            self.player.enqueue_next(&uri)?;
        }else {
            return Err("this item does not exist!".into());
        }
        Ok(())
    }
}


#[test]
fn item_add_test() {
    let mut q = Queue::new().unwrap();
    for _ in 0..5 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::Queue, by_human: false });
    }
    q.clear();
    for _ in 0..1 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    // q.clear_played();
    // for _ in 0..3 {
    // q.remove_item(0).inspect_err(|e| println!("{e:?}"));
    // }
    for _ in 0..2 {
        q.items.push(QueueItem { item: QueueItemType::Test, state: QueueState::None, source: QueueSource::Queue, by_human: false });
    }
    q.add_item_next(QueueItemType::Test, QueueSource::File);

    dbg!(&q.items, &q.items.len());
}

#[test]
fn test_() {
    let mut q = Queue::new().unwrap();
    for _ in 0..100 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::Queue, by_human: false });
    }
    for _ in 0..2 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    q.add_item_next(QueueItemType::Test, QueueSource::Queue);

    dbg!(&q.items, &q.items.len());

}

#[test]
fn move_test() {
    let mut q = Queue::new().unwrap();
    // for _ in 0..1 {
    //     q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::Queue, by_human: false });
    // }
    for _ in 0..5 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    q.add_item(QueueItemType::Test, QueueSource::Library, true).unwrap();
    dbg!(&q.items, &q.items.len());

    q.move_to(3).inspect_err(|e| {dbg!(e);});
    dbg!(&q.items, &q.items.len());
}