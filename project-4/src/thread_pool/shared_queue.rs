use std::{sync::{Arc, Mutex, mpsc::{Receiver, Sender, channel}}, thread};

use crate::error::Result;
use super::ThreadPool;

/// todo
pub struct SharedQueueThreadPool {
    queue: Sender<Box<dyn FnOnce() + Send + 'static>>,
    shared: Arc<ThreadPoolSharedData>
}

struct ThreadPoolSharedData {
    job: Mutex<Receiver<Box<dyn FnOnce() + Send + 'static>>>
}

struct Sentinel {
    data: Arc<ThreadPoolSharedData>,
    active: bool
}

impl ThreadPool for SharedQueueThreadPool {
    fn new(threads: u32) -> Result<Self> where Self:Sized {
        let (tx, rx) = channel::<Box<dyn FnOnce() + Send + 'static>>();
        
        let shared = Arc::new(ThreadPoolSharedData{
            job: Mutex::new(rx)
        });

        for _ in 0..threads {
            Self::create_worker(shared.clone())?;
        }
        
        Ok(SharedQueueThreadPool{
            queue: tx,
            shared: shared
        })
    }
    fn spawn<F>(&self, job: F) where F: FnOnce() + Send + 'static {
        self.queue.send(Box::new(job)).expect(
            "Send job error"
        );
    }
}

impl SharedQueueThreadPool {
    fn create_worker(shared: Arc<ThreadPoolSharedData>) -> Result<()> {
        let thread_builder = thread::Builder::new();
        thread_builder.spawn(move|| {
            let shared = Sentinel::new(shared);

            loop {
                let job = match {
                    let receiver = shared.data.job.lock().expect(
                        "Can't lock receiver"
                    );
                    receiver.recv()
                } {
                    Ok(job) => job,
                    Err(_) => break
                };

                job();
            }

            shared.cancel();
        })?;
        Ok(())
    }
}

impl Sentinel {
    fn new(data: Arc<ThreadPoolSharedData>) -> Self {
        Sentinel {
            data: data,
            active: true
        }
    }
    fn cancel(mut self) {
        self.active = false;
    }
}

impl Drop for Sentinel {
    fn drop(&mut self) {
        if self.active {
            SharedQueueThreadPool::create_worker(self.data.clone()).expect(
                "Sentinel recovery failed"
            );
        }
    }
}