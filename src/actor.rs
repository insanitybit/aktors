use async_trait::async_trait;
use std::fmt::Debug;

use tokio::sync::mpsc::{channel, Receiver, Sender, error::TryRecvError};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[async_trait]
pub trait Actor<M> {
    async fn route_message(&mut self, message: M);
    fn get_actor_name(&self) -> &str;
    fn close(&mut self);
}

pub struct Router<A, M>
    where A: Actor<M>,
{
    actor_impl: Option<A>,
    receiver: Receiver<M>,
    inner_rc: Arc<AtomicUsize>,
    queue_len: Arc<AtomicUsize>,
}

impl<A, M> Router<A, M>
    where A: Actor<M>,
{
    pub fn new(
        actor_impl: A,
        receiver: Receiver<M>,
        inner_rc: Arc<AtomicUsize>,
        queue_len: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            actor_impl: Some(actor_impl),
            receiver,
            inner_rc,
            queue_len,
        }
    }
}

pub async fn route_wrapper<A, M>(mut router: Router<A, M>)
    where A: Actor<M>,
{
    let mut empty_tries = 0;

    loop {
        tokio::task::yield_now().await;
        let msg = tokio::time::timeout(
            Duration::from_millis(0 + 10),
            router.receiver.recv()
        ).await;

        match msg {
            Ok(Some(msg)) => {
                empty_tries = 0;

                router.queue_len.clone().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

                router.actor_impl.as_mut().expect("route_message actor_impl was None").route_message(msg).await;

                let inner_rc = router.inner_rc.load(Ordering::SeqCst);
                let queue_len = router.queue_len.load(Ordering::SeqCst);

                if queue_len > 0 {
                    continue
                }
                if inner_rc <= 1 {
                    if let Some(actor_impl) = router.actor_impl.as_mut() {
                        actor_impl.close();
                    }
                    router.receiver.close();
                    router.actor_impl = None;
                    break;
                }
            }
            // Queue was empty for timeout duration
            Err(_) => {
                if empty_tries > 90 {
                    empty_tries = 0;
                }
                empty_tries += 1;

                let inner_rc = router.inner_rc.load(Ordering::SeqCst);
                let queue_len = router.queue_len.load(Ordering::SeqCst);

                if queue_len > 0 {
                    continue
                }

                if inner_rc <= 1 {
                    if let Some(ref mut actor_impl) = router.actor_impl.as_mut() {
                        actor_impl.close();
                    }
                    router.receiver.close();
                    router.actor_impl = None;
                    break;
                }
            }
            // Disconnected
            Ok(None) => {
                empty_tries = 0;
                let inner_rc = router.inner_rc.load(Ordering::SeqCst);
                let queue_len = router.queue_len.load(Ordering::SeqCst);

                if queue_len > 0{
                    continue
                }

                if inner_rc <= 1 {
                    if let Some(ref mut actor_impl) = router.actor_impl.as_mut() {
                        actor_impl.close();
                    }
                    router.receiver.close();
                    router.actor_impl = None;
                    break;
                }
            }
        }
        tokio::task::yield_now().await;
    }
}
