use std;

use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

use futures;
use futures::future::*;

use fibers::Spawn;

use uuid::Uuid;

pub trait Actor<M: Send>: Send {
    fn on_message(&mut self, msg: M);
}

pub trait ActorRef<M: Send>: Clone {
    fn send(&mut self, msg: M);
}

pub struct Supervisor<A, H, F, M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M>,
          M: Send
{
    map: std::collections::HashMap<String, (WorkerRef<M>, ChildSpec<A, H, F, M>)>,
}

impl<A, H, F, M> Supervisor<A, H, F, M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M> + 'static,
          M: Send + 'static
{
    pub fn new(handle: H, child_specs: Vec<ChildSpec<A, H, F, M>>) -> Supervisor<A, H, F, M> {
        let mut map = std::collections::HashMap::new();

        for spec in child_specs {
            let child = (spec.start)(handle.clone());
            let actor = actor_of(handle.clone(), child);
            map.insert(spec.key.clone(), (actor, spec));
        }

        Supervisor { map: map }
    }
}

impl<A, H, F, M> Actor<SupervisorMessage<M>> for Supervisor<A, H, F, M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M>,
          M: Send
{
    fn on_message(&mut self, msg: SupervisorMessage<M>) {
        let id = msg.id.clone();
        println!("got id {}", id);
        if let Some(val) = self.map.get_mut(&id) {
            let &mut (ref mut actor, _) = val;

            let msg = msg.msg;
            actor.send(msg)

            //                match result {
            //                    Err(_) => {
            //                        *actor = actor_of(self.handle.clone(),
            //                                          (childspec.start)(self.handle.clone()))
            //                    }
            //                    _ => (),
            //                }
        }
    }
}

pub struct WorkerRef<M>
    where M: Send
{
    sender: Sender<M>,
    receiver: Receiver<M>,
    id: String,
}

// wtf, why does this work and #[derive(Clone)] not?
impl<M> Clone for WorkerRef<M>
    where M: Send
{
    fn clone(&self) -> Self {
        WorkerRef {
            sender: self.sender.clone(),
            receiver: self.receiver.clone(),
            id: self.id.clone(),
        }
    }
}

#[derive(Clone)]
pub enum Restart {
    Transient,
    Persistent,
    Temporary,
}

#[derive(Clone)]
pub enum Shutdown {
    Eventually,
    Immediately,
    Brute,
}

#[derive(Clone)]
pub enum ActorKind {
    System,
    Worker,
    Supervisor,
}

#[derive(Clone)]
pub struct ChildSpec<A, H, F, M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M>,
          M: Send
{
    key: String,
    start: F,
    restart: Restart,
    shutdown: Shutdown,
    kind: ActorKind,
    _ph: std::marker::PhantomData<(H, M)>,
}

impl<A, H, F, M> ChildSpec<A, H, F, M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M>,
          M: Send
{
    pub fn new(key: String,
               start: F,
               restart: Restart,
               shutdown: Shutdown,
               kind: ActorKind)
               -> ChildSpec<A, H, F, M> {
        ChildSpec {
            key: key,
            start: start,
            restart: restart,
            shutdown: shutdown,
            kind: kind,
            _ph: std::marker::PhantomData,
        }
    }
}

pub struct SupervisorMessage<M> {
    id: String,
    msg: M,
}

impl<M> ActorRef<M> for WorkerRef<M>
    where M: Send
{
    fn send(&mut self, msg: M) {
        self.sender.send(msg).unwrap();
    }
}

pub fn supervised_actor_of<A, H, F, M>(exec: H,
                                       mut actor: A,
                                       _: ChildSpec<A, H, F, M>)
                                       -> WorkerRef<M>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> A,
          A: Actor<M> + 'static,
          M: Send + 'static
{
    let (sender, receiver) = unbounded();
    let recvr = receiver.clone();
    exec.spawn(futures::lazy(move || {
        loop_fn(0, move |_| match recvr.try_recv() {
            Ok(msg) => {
                actor.on_message(msg);

                Ok::<_, _>(Loop::Continue(0))
            }
            Err(TryRecvError::Disconnected) => Ok::<_, _>(Loop::Break(())),
            Err(TryRecvError::Empty) => Ok::<_, _>(Loop::Continue(0)),
        })
    }));

    WorkerRef {
        sender: sender,
        receiver: receiver,
        id: Uuid::new_v4().to_string(),
    }
}


pub fn actor_of<A, H, M>(exec: H, mut actor: A) -> WorkerRef<M>
    where H: Send + Spawn + Clone + 'static,
          M: Send + 'static,
          A: Actor<M> + 'static
{
    let (sender, receiver) = unbounded();
    let recvr = receiver.clone();
    exec.spawn(futures::lazy(move || {
        loop_fn(0, move |_| match recvr.try_recv() {
            Ok(msg) => {
                actor.on_message(msg);

                Ok::<_, _>(Loop::Continue(0))
            }
            Err(TryRecvError::Disconnected) => Ok::<_, _>(Loop::Break(())),
            Err(TryRecvError::Empty) => Ok::<_, _>(Loop::Continue(0)),
        })
    }));

    WorkerRef {
        sender: sender,
        receiver: receiver,
        id: Uuid::new_v4().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::any::Any;

    use fibers::{Executor, ThreadPoolExecutor};

    struct MyActor<H>
        where H: Send + Spawn + Clone + 'static
    {
        handle: H,
        id: String,
    }

    impl<H> MyActor<H>
        where H: Send + Spawn + Clone + 'static
    {
        pub fn new(h: H) -> MyActor<H> {
            MyActor {
                handle: h,
                id: Uuid::new_v4().to_string(),
            }
        }
    }


    impl<H> Actor<u64> for MyActor<H>
        where H: Send + Spawn + Clone + 'static
    {
        fn on_message(&mut self, msg: u64) {
            let number = msg;
            if number % 1000 == 0 {
                println!("{} got {}", self.id, number);
            }

            //                if *number == 0 {panic!("zero!")};
            let new_actor = MyActor::new(self.handle.clone());
            let actor_ref = actor_of(self.handle.clone(), new_actor);
            actor_ref.sender.send(number + 1);
        }
    }

    #[test]
    fn supervisors() {
        let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
        let handle = system.handle();

        let child_spec = ChildSpec::new("worker child".to_owned(),
                                        move |handle| MyActor::new(handle),
                                        Restart::Temporary,
                                        Shutdown::Eventually,
                                        ActorKind::Worker);

        let mut supervisor_ref = actor_of(handle.clone(),
                                          Supervisor::new(handle.clone(), vec![child_spec]));

        supervisor_ref.send(SupervisorMessage {
                                id: "worker child".to_owned(),
                                msg: 1000000,
                            });

        drop(supervisor_ref);

        let _ = system.run();
    }

    #[test]
    fn test_name() {
        let system = ThreadPoolExecutor::with_thread_count(2).unwrap();

        let actor = MyActor::new(system.handle());
        let mut actor_ref = actor_of(system.handle(), actor);
        actor_ref.send(0 as u64);
        drop(actor_ref);

        let _ = system.run();
    }
}

