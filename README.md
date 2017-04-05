Rust actor library with a bit of inspiration from Akka/Pykka

Built with the 'fibers' crate.

# Example

# Actors

We first declare our MyActor struct like any other. We take a 'handle' so that we can spawn
child actors. (Note that the id is entirely unnecessary, an ActorRef will have its own unique identifier.)

```rust
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
```

We then impl Actor for MyActor. We get a message (Box<Any + Send>) and we downcast it to the type we expect.

We then create a child actor and send it its own message (our message + 1).

This will loop (concurrently, yielding after each on_message call) forever, counting eternally.

```rust
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
```

To actually execute the actor we need to create an Execution context, and use the actor_of function.

```rust
    let system = ThreadPoolExecutor::with_thread_count(2).unwrap();

    let actor = MyActor::new(system.handle());
    let mut actor_ref = actor_of(system.handle(), Box::new(actor));
    actor_ref.send(Box::new(0 as u64));
    drop(actor_ref);

    let _ = system.run();
```

# Supervisors

Using the above MyActor we create a ChildSpec, which will return a MyActor. The supervisor uses
this function to generate new actors and replace dead ones.

We then create the predefined Supervisor, and pass it to actor_of.

We can then send messages to the supervisor, using the SuperVisorMessage structure. We provide
the name we gave to the child, which the supervisor uses internally to route messages.

(Currently supervisors do not catch panics - this will change)

```rust
        let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
        let handle = system.handle();

        let child_spec = ChildSpec::new("worker child".to_owned(),
                                        move |handle| Box::new(MyActor::new(handle)) as Box<Actor>,
                                        Restart::Temporary,
                                        Shutdown::Eventually,
                                        ActorKind::Worker);

        let mut supervisor_ref =
            actor_of(handle.clone(),
                     Box::new(Supervisor::new(handle.clone(), vec![child_spec])) as Box<Actor>);

        supervisor_ref.send(Box::new(SupervisorMessage {
            id: "worker child".to_owned(),
            msg: Box::new(1000000 as u64),
        }));


        drop(supervisor_ref);

        let _ = system.run();
```
