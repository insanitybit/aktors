use async_trait::async_trait;
use std::fmt::Debug;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait Message {
    fn is_release(&self) -> bool;
}

#[async_trait]
pub trait Actor<M: Message> {
    async fn route_message(&mut self, message: M);
    fn close(&mut self);
}

pub struct Router<A, M>
    where A: Actor<M>,
          M: Message,
{
    actor_impl: A,
    receiver: Receiver<M>,
    inner_rc: Arc<AtomicUsize>,
    queue_len: Arc<AtomicUsize>,
}

impl<A, M> Router<A, M>
    where A: Actor<M>,
          M: Message,
{
    pub fn new(
        actor_impl: A,
        receiver: Receiver<M>,
        inner_rc: Arc<AtomicUsize>,
        queue_len: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            actor_impl,
            receiver,
            inner_rc,
            queue_len,
        }
    }
}

pub async fn route_wrapper<A, M>(mut router: Router<A, M>)
    where A: Actor<M>,
          M: Message,
{
    tokio::task::yield_now().await;

    while let Some(msg) = router.receiver.recv().await {
        router.queue_len.clone().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if msg.is_release() {
            tokio::task::yield_now().await;
        }

        router.actor_impl.route_message(msg).await;
        tokio::task::yield_now().await;

        if router.inner_rc.load(Ordering::SeqCst) == 1 {
            let queue_len = router.queue_len.load(Ordering::SeqCst);
            dbg!(queue_len);
            if queue_len == 0 {
                tokio::task::yield_now().await;
                router.actor_impl.close();
            }
        }
    }
}
