use crossbeam::queue;

use crate::music_storage::queue::{Queue, QueueError};

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
            let QueueCommandInput {
                res_rx: res_tx,
                command,
            } = queue_mail.recv().await.unwrap();
            match command {
                QueueCommand::Append(item) => {
                    queue.add_item(item);
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Next => {
                    res_tx
                        .send(QueueResponse::Next(queue.next(false)))
                        .await
                        .unwrap();
                }
                QueueCommand::Prev => {
                    let prev = queue
                        .prev()
                        .map_or(Err(QueueError::EmptyPlayed), |s| Ok(s.clone()));
                    res_tx
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
                    res_tx.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::NowPlaying => {
                    let item = queue.current().map(|t| t.clone());
                    res_tx.send(QueueResponse::Item(item)).await.unwrap();
                }
                QueueCommand::Get => {
                    let mut out = queue.queue.clone();
                    out.append(&mut queue.up_next_visible.clone());
                    res_tx.send(QueueResponse::GetAll(out)).await.unwrap();
                }
                QueueCommand::Clear => {
                    queue.clear();
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::ClearExceptQueue => {
                    queue.up_next_visible.clear();
                    queue.up_next_invisible.clear();
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::RemoveQueue(index) => {
                    res_tx
                        .send(QueueResponse::Item(queue.remove(index)))
                        .await
                        .unwrap();
                }
                QueueCommand::AddAfterNP(item) => {
                    queue.add_after_np(item);
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::MoveTo(index) => {
                    res_tx
                        .send(QueueResponse::Move(queue.move_to(index)))
                        .await
                        .unwrap();
                }
                #[allow(clippy::unit_arg)]
                QueueCommand::AddUpNext(item) => res_tx
                    .send(QueueResponse::Empty(Ok(queue.add_up_next(item))))
                    .await
                    .unwrap(),
                QueueCommand::AddUpNextInvis(items) => {
                    queue.add_up_next_invis(items);
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Info => res_tx
                    .send(QueueResponse::Info {
                        looping: queue.looping.clone(),
                        shuffle: queue.shuffle.clone(),
                        up_next_limit: queue.up_next_limit.clone(),
                        pull_location: queue.pull_location.clone(),
                    })
                    .await
                    .unwrap(),
                QueueCommand::Shuffle(shuffle) => {
                    queue.shuffle = shuffle;
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::SetPullLocation(location) => {
                    queue.pull_location = Some(location);
                    res_tx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
            }
        }
    }
}
