use std;
use std::any::Any;
use std::panic;
use std::panic::AssertUnwindSafe;

use chashmap::CHashMap;

use two_lock_queue::{unbounded, Sender, Receiver, TryRecvError};

use futures;
use futures::future::*;

use futures::{Future, future};
use fibers::{Executor, ThreadPoolExecutor, Spawn};

use uuid::Uuid;

pub trait Actor: Send {
    fn on_message(&mut self, msg: Box<Any + Send>);
}

pub trait ActorRef: Clone {
    fn send(&mut self, msg: Box<Any + Send>);
}

pub struct Supervisor<H, F>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor> {
    map: std::collections::HashMap<String, (WorkerRef, ChildSpec<H, F>)>,
    handle: H
}

impl<H, F> Supervisor<H, F>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor>  {
    pub fn new(handle: H, child_specs: Vec<ChildSpec<H, F>>) -> Supervisor<H, F>
    {
        let mut map = std::collections::HashMap::new();

        for spec in child_specs {
            let child = (spec.start)(handle.clone());
            let actor = actor_of(handle.clone(), child);
            map.insert(spec.key.clone(), (actor, spec));
        }

        Supervisor {
            map: map,
            handle: handle
        }
    }
}

impl<H, F> Actor for Supervisor<H, F>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor>  {
    fn on_message(&mut self, msg: Box<Any + Send>) {
        if let Ok(msg) = msg.downcast::<SupervisorMessage>() {
            let id = msg.id.clone();
            println!("got id {}", id);
            if let Some(val) = self.map.get_mut(&id) {
                let &mut (ref mut actor, ref childspec) = val;

                let msg = msg.msg;
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    actor.send(msg);
                }));

                match result {
                    Err(_) => *actor = actor_of(self.handle.clone(),
                                                (childspec.start)(self.handle.clone())),
                    _ => ()
                }
            }
        } else {
            panic!("downcast error");
        }
    }
}

#[derive(Clone)]
pub struct WorkerRef {
    sender: Sender<Box<Any + Send>>,
    receiver: Receiver<Box<Any + Send>>,
    id: String,
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
pub struct ChildSpec<H, F>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor>
{
    key: String,
    start: F,
    restart: Restart,
    shutdown: Shutdown,
    kind: ActorKind,
    _ph: std::marker::PhantomData<H>,
}

impl<H, F> ChildSpec<H, F>
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor>
{
    pub fn new(    key: String,
                   start: F,
                   restart: Restart,
                   shutdown: Shutdown,
                   kind: ActorKind) -> ChildSpec<H, F> {
        ChildSpec {
            key: key,
            start: start,
            restart: restart,
            shutdown: shutdown,
            kind: kind,
            _ph: std::marker::PhantomData
        }
    }
}
struct SupervisorMessage {
    id: String,
    msg: Box<Any + Send>,
}

impl ActorRef for WorkerRef {
    fn send(&mut self, msg: Box<Any + Send>) {
        self.sender.send(msg).unwrap();
    }
}

pub fn supervised_actor_of<H, F>(exec: H, mut actor: Box<Actor>, policy: ChildSpec<H, F>) -> WorkerRef
    where H: Send + Spawn + Clone + 'static,
          F: Send + 'static + Fn(H) -> Box<Actor>
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


pub fn actor_of<H>(exec: H, mut actor: Box<Actor>) -> WorkerRef
    where H: Send + Spawn + Clone + 'static
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


    impl<H> Actor for MyActor<H>
        where H: Send + Spawn + Clone + 'static
    {
        fn on_message(&mut self, msg: Box<Any + Send>) {
            if let Some(number) = msg.downcast_ref::<u64>() {
                if number % 1000 == 0 {
                    println!("{} got {}", self.id, number);
                }

                //                if *number == 0 {panic!("zero!")};
                let new_actor = Box::new(MyActor::new(self.handle.clone())) as Box<Actor>;
                let actor_ref = actor_of(self.handle.clone(), new_actor);
                actor_ref.sender.send(Box::new(number + 1));
                drop(actor_ref);
            } else {
                panic!("downcast error");
            }
        }
    }

    #[test]
    fn supervisors() {

        let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
        let handle = system.handle();

        let child_spec = ChildSpec::new(
            "worker child".to_owned(),
            move |handle| Box::new(MyActor::new(handle)) as Box<Actor>,
            Restart::Temporary,
            Shutdown::Eventually,
            ActorKind::Worker
        );

        let mut supervisor_ref = actor_of(handle.clone(), Box::new(Supervisor::new(handle.clone(), vec![child_spec])) as Box<Actor>);

        supervisor_ref.send(
            Box::new(
                SupervisorMessage{
                    id: "worker child".to_owned(),
                    msg: Box::new(1000000 as u64)}
            ));

        let mut other_ref = supervisor_ref.clone();

        drop(supervisor_ref);

        other_ref.send(
            Box::new(
                SupervisorMessage{
                    id: "worker child".to_owned(),
                    msg: Box::new(1000000 as u64)}
            ));

        drop(other_ref);

        let _ = system.run();
    }

    //    #[test]
    //    fn test_name() {
    //
    //        let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
    //
    //        let actor = ;
    //        let actor_ref = actor_of(system.handle(), actor);
    //        actor_ref.send(Box::new(0 as u64));
    //        drop(actor_ref);
    //
    //        let actor = Box::new(MyActor::new(system.handle())) as Box<Actor>;
    //        let actor_ref = actor_of(system.handle(), actor);
    //        actor_ref.send(Box::new(1000000 as u64));
    //        drop(actor_ref);
    //
    //        let _ = system.run();
    //    }
}
