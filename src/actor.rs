use async_trait::async_trait;
use std::fmt::Debug;

use tokio::sync::mpsc::{channel, Receiver, Sender};


#[async_trait]
pub trait Actor<M> {
    async fn route_message(&mut self, message: M);
}

pub struct Router<A, M>
    where A: Actor<M>,
{
    actor_impl: A,
    receiver: Receiver<M>,
}

pub async fn route_wrapper<A, M>(mut router: Router<A, M>)
    where A: Actor<M>,
{
    tokio::task::yield_now().await;

    while let Some(msg) = router.receiver.recv().await {
        router.actor_impl.route_message(msg).await;
    }
}