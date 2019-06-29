#[cfg(test)]

use crate::futures::Future;
use futures::lazy;
use tokio::runtime::Runtime;
use std::string::ToString;
use crate::{PrintLoggerActor, PrintLogger, CountLogger, CountLoggerActor};

#[test]
fn test_aktor() {
    let mut rt: Runtime = Runtime::new().unwrap();

    rt.spawn(lazy(|| -> Result<(), ()> {
        let logger = PrintLogger {};
        let counter = CountLogger::default();

        let mut log_actor = PrintLoggerActor::new(logger);
        let mut count_actor = CountLoggerActor::new(counter);

        for i in 0 .. 10 {
            log_actor.info(format!("On info iteration: {}", i), count_actor.clone());
            log_actor.error(format!("On error iteration: {}", i), count_actor.clone());
        }

        Ok(())
    }));

    rt.shutdown_on_idle().wait().unwrap();
}