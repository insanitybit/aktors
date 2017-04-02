p# aktors
Rust actor library with a bit of inspiration from Akka/Pykka


# Example

Here we create a supervisor, which spawns a single child. We then send a message to the child using the name
we provided.

```rust

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


        drop(supervisor_ref);

        let _ = system.run();
```