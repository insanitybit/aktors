#[cfg(test)]

use tokio::runtime::Runtime;
use std::string::ToString;
use crate::{PrintLoggerActor, PrintLogger};
use tokio_async_await::compat::backward;

#[test]
fn test_aktor() {
    async fn test_aktor_impl() -> Result<(), ()> {
        let logger = PrintLogger {};
        let mut log_actor = PrintLoggerActor::new(logger);

        await!(log_actor.info("info log".to_string()));
        await!(log_actor.error("error!!".to_string()));

        Ok(())
    };

    let mut rt: Runtime = Runtime::new().unwrap();

    rt.block_on(backward::Compat::new(test_aktor_impl()));


    /*let system = ThreadPoolExecutor::with_thread_count(2).unwrap();
    let logger = PrintLogger {};
    let log_actor = PrintLoggerActor::new(system.handle(), logger);
    // These two functions return immediately
    // None of our written code had to use threads or fibers or futures or anything,
    // concurrency for free.

    log_actor.info("info log");
    log_actor.error("error!!");
    system.run();*/
}