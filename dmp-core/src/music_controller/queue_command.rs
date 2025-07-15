use crate::music_storage::queue::{Queue, QueueError, QueueItemType};

use super::{
    controller::{Controller, QueueCommand, QueueResponse},
    controller_handle::QueueCommandInput,
};

impl Controller {
    pub(super) async fn queue_loop(
        mut queue: Queue,
        queue_mail: async_channel::Receiver<QueueCommandInput>,
    ) {
        while true {
            let QueueCommandInput { res_rx, command } = queue_mail.recv().await.unwrap();
            match command {
                QueueCommand::Append(item, by_human) => {
                    match item.item {
                        QueueItemType::Song(song) => {
                            queue.add_item(QueueItemType::Song(song), by_human)
                        }
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
                    let item = queue.queue.get(index).map_or(
                        Err(QueueError::OutOfBounds {
                            index,
                            len: queue.queue.len(),
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
                        .send(QueueResponse::GetAll(queue.queue.clone()))
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
                        QueueItemType::Song(song) => {
                            queue.add_item_next(QueueItemType::Song(song));
                        }
                        QueueItemType::Album { .. } => {
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
