use font::opentype::tables::font_variations::InstanceFlags;
use uuid::Uuid;

use crate::{music_player::Player, music_storage::library::{Album, MusicLibrary, Song, URI}};
use std::{error::Error, ops::Add, path::Path, sync::{Arc, RwLock}, thread::sleep, time::Duration};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum QueueState {
    Played,
    Current,
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
            state: QueueState::NoState,
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
    fn dbg_items(&self) {
        dbg!(self.items.iter().map(|item| item.item.clone() ).collect::<Vec<QueueItemType>>(), self.items.len());
    }
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(
            Queue {
            player: Player::new()?,
            name: String::new(),
            items: Vec::new()
            }
        )
    }

    pub fn current_index(&mut self/* , max: usize */) -> Option<usize> {
        let mut e  = self.items.iter().filter(|song| song.state == QueueState::Played ).collect::<Vec<&QueueItem>>().len();
        // TODO: make the max number of past songs modular
        while e > 50 {
            self.items.remove(0);
            e -=1;
        }
        if e == 0 {
            None
        }else {
            Some(e - 1)
        }
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
        let mut i: usize = 0;
        let ind =  self.current_index();

        self.items = self.items.iter().enumerate().map(|(j, item_)| {
            let mut item_ = item_.to_owned();
            // get the index of the current AddHere item and give it to i
            if item_.state == AddHere {
                i = j - ind.unwrap_or(0);
                item_.state = NoState;
            } else if item_.state == Current {
                i = j - ind.unwrap_or(0);
            }
            item_
        }).collect::<Vec<QueueItem>>();

        let pos = ind.unwrap_or(0) + i  + if !self.is_empty() || (self.is_empty() && ind == None) { 0 } else { 1 };
        // dbg!(&pos, &i, &ind);
        self.items.insert(
            pos,
            QueueItem {
                item: item.clone(),
                state: if pos == self.items.len() && i == 0 {
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
        let ind_ =  self.current_index();
        let ind =  ind_.unwrap_or(0);
        let empty = self.is_empty();

        self.items.insert(
            (ind + if !empty && ind_ == None { 1 } else { 2 }),
            QueueItem {
                item,
                state: if empty {
                    Current
                }else if self.items.get(ind + 1).is_none() || (!self.contains_state(AddHere) && self.items.get(ind + 1).is_some()) {
                    AddHere
                }else {
                    NoState
                },
                source,
                by_human: true
            }
        )
    }

    pub fn remove_item(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        use QueueState::*;
        let remove_index: usize = (if let Some(current_index) =  self.current_index() { dbg!(&current_index); current_index } else { 0 } + index );

        // dbg!(/*&remove_index, self.current_index(), &index,*/ &self.items[remove_index]);

        if remove_index < self.items.len() {
            // update the state of the next item to replace the item being removed
            if self.items.get(remove_index + 1).is_some() {
                self.items[remove_index + 1].state = self.items[remove_index].state;
            }else if self.items[remove_index].state != Current {
                self.items[remove_index - 1].state = self.items[remove_index].state;
            }
            self.items[remove_index].state = NoState;
            self.items.remove(remove_index);
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
        let ind = match self.current_index() {
            Some(e) => e,
            None =>  return Err("nothing to clear!".into())
        };
        let empty = self.is_empty();

        if !empty {
            index += ind;
        }else {
            index -=1
        }

        if !empty && index < self.items.len() {
            let i = self.items[index].clone();
            self.items.retain(|item| item.state == QueueState::Played || *item == i );
            self.items[ind+1].state = QueueState::Current
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
        let empty = self.is_empty();
        let nothing_error = Err("Nothing in the queue to move to!".into());
        let ind = self.current_index().unwrap_or(0);
        let index = if !empty { index + ind } else { return nothing_error; };

        if !empty && index < self.items.len() -1 {
            let position = self.player.position();
            if position.is_some_and(|dur| !dur.is_zero() ) {
                self.items[ind].state = QueueState::Played;
            }

            let to_item = self.items[index].clone();
            let ind = self.current_index().unwrap_or(0);

            loop {
                if self.items[ind].item != to_item.item {
                    if let Err(e) = self.remove_item(0) {
                        dbg!(&e); self.dbg_items(); return Err(e);
                    }
                // dbg!(&to_item.item, &self.items[ind].item);
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
    dbg!(1);
    for _ in 0..1 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::File, by_human: false });
    }
    dbg!(2);

    // q.clear();
    dbg!(3);

    for _ in 0..5 {
        // dbg!("tick!");
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
        // dbg!(&q.items, &q.items.len());
    }
    dbg!(4);
    dbg!(&q.items, &q.items.len());

    // q.clear_played();
    for _ in 0..1 {
    q.remove_item(0).inspect_err(|e| println!("{e:?}"));
    }
    // for _ in 0..2 {
    //     q.items.push(QueueItem { item: QueueItemType::Test, state: QueueState::NoState, source: QueueSource::Library, by_human: false });
    // }
    // dbg!(5);

    // q.add_item_next(QueueItemType::Test, QueueSource::File);
    // dbg!(6);

    dbg!(&q.items, &q.items.len());
}

#[test]
fn test_() {
    let mut q = Queue::new().unwrap();
    for _ in 0..1 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::File, by_human: false });
    }
    for _ in 0..2 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    q.add_item_next(QueueItemType::Test, QueueSource::File);

    dbg!(&q.items, &q.items.len());

}

#[test]
fn move_test() {
    let mut q = Queue::new().unwrap();
    for _ in 0..1 {
        q.items.push(QueueItem { item: QueueItemType::Song(Uuid::new_v4()), state: QueueState::Played, source: QueueSource::File, by_human: false });
    }
    for _ in 0..5 {
        q.add_item(QueueItemType::Song(Uuid::new_v4()), QueueSource::Library, true).unwrap();
    }
    // q.add_item(QueueItemType::Test, QueueSource::Library, true).unwrap();
    dbg!(&q.items, &q.items.len());

    q.move_to(3).inspect_err(|e| {dbg!(e);});
    dbg!(&q.items, &q.items.len());
    // q.dbg_items();
}