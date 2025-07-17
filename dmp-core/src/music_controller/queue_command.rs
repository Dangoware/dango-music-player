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
            let QueueCommandInput { res_rx, command } = queue_mail.recv().await.unwrap();
            match command {
                QueueCommand::Append(item) => {
                    queue.add_item(item);
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::Next => {
                    let next = queue
                        .next()
                        .map_or(Err(QueueError::NoNext), |s| Ok(s.clone()));
                    res_rx
                        .send(QueueResponse::Next(next.clone()))
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
                QueueCommand::RemoveQueue(index) => {
                    res_rx
                        .send(QueueResponse::Item(queue.remove_queue(index)))
                        .await
                        .unwrap();
                }
                QueueCommand::RemoveNextUp(index) => {
                    res_rx
                        .send(QueueResponse::Item(queue.remove_next_up(index)))
                        .await
                        .unwrap();
                }
                QueueCommand::AddAfterNP(item) => {
                    queue.add_after_np(item);
                    res_rx.send(QueueResponse::Empty(Ok(()))).await.unwrap();
                }
                QueueCommand::MoveTo(index) => {
                    res_rx
                        .send(QueueResponse::Move(queue.move_to(index)))
                        .await
                        .unwrap();
                }
            }
        }
    }
}
