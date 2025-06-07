use crate::music_storage::{
    library::Song,
    queue::{Queue, QueueError, QueueItemType},
};

use super::{
    controller::{Controller, QueueCommand, QueueResponse},
    controller_handle::QueueCommandInput,
    queue::{QueueAlbum, QueueSong},
};

impl Controller {
    pub(super) async fn queue_loop(
        mut queue: Queue<QueueSong, QueueAlbum>,
        queue_mail: async_channel::Receiver<QueueCommandInput>,
    ) {
        while true {
            let QueueCommandInput { res_rx, command } = queue_mail.recv().await.unwrap();
            match command {
                QueueCommand::Append(item, by_human) => {
                    match item.item {
                        QueueItemType::Single(song) => queue.add_item(song, by_human),
                        _ => unimplemented!(),
                    }
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Next => {
                    let next = queue
                        .next()
                        .map_or(Err(QueueError::NoNext), |s| Ok(s.clone()));
                    res_rx
                        .send(QueueResponse::Item(next.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Prev => {
                    let prev = queue
                        .prev()
                        .map_or(Err(QueueError::EmptyPlayed), |s| Ok(s.clone()));
                    res_rx
                        .send(QueueResponse::Item(prev.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::GetIndex(index) => {
                    let item = queue.items.get(index).map_or(
                        Err(QueueError::OutOfBounds {
                            index,
                            len: queue.items.len(),
                        }),
                        |s| Ok(s.clone()),
                    );
                    res_rx.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().map(|t| t.clone());
                    res_rx.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::Get => {
                    res_rx
                        .send(QueueResponse::GetAll(queue.items.clone()))
                        .await
                        .unwrap();
                }
                QueueCommand::Clear => {
                    queue.clear();
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Remove(index) => {
                    res_rx
                        .send(QueueResponse::Item(queue.remove_item(index)))
                        .await
                        .unwrap();
                }
                QueueCommand::PlayNext(item, by_human) => {
                    match item.item {
                        QueueItemType::Single(song) => {
                            queue.add_item_next(song);
                        }
                        QueueItemType::Multi(album) => {
                            unimplemented!()
                        }
                    };
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::MoveTo(index) => {
                    res_rx
                        .send(QueueResponse::Empty(queue.move_to(index)))
                        .await
                        .unwrap();
                }
            }
        }
    }
}
