
use std::time::Duration;
use tokio_core::reactor::{Core, Handle, Remote};

use channel::{unbounded as chan};
use futures::future::*;
use std;

pub struct SystemActor {
    remote: Remote
}

impl SystemActor {
    pub fn new() -> SystemActor {

        let (sender, receiver) = chan();

        std::thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let remote = core.remote();
            sender.send(remote);

            loop {
                core.turn(Some(Duration::from_secs(1)));
            }

        });

        let remote = receiver.recv().unwrap();

        SystemActor {
            remote
        }
    }

    pub fn spawn<F, R>(&self, f: F)
        where F: FnOnce(&Handle) -> R + Send + 'static,
              R: IntoFuture<Item=(), Error=()>,
              R::Future: 'static,
    {
        self.remote.spawn(f);
    }
}