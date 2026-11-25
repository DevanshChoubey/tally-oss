//! The API worker: one background thread owns the [`Api`], so the UI thread never blocks on the
//! store or the engine (spec section 7, rule D1).
//!
//! A screen calls [`ApiWorker::run`] with a closure over `&mut Api` and keeps the returned
//! [`Ticket`]. Each frame it calls [`Ticket::poll`]; the result arrives once the worker has run the
//! job. Jobs run one at a time, in the order they were submitted, so a later read sees every
//! earlier change.

use std::fmt;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;

use ledgerly_api::Api;

/// One unit of work for the worker thread.
type Job = Box<dyn FnOnce(&mut Api) + Send>;

/// Owns the background thread that runs every [`Api`] call.
pub struct ApiWorker {
    /// Sends jobs to the thread. Dropped first on shutdown, which ends the thread's loop.
    jobs: Option<Sender<Job>>,
    /// The thread, joined on drop.
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for ApiWorker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiWorker")
            .field("running", &self.jobs.is_some())
            .finish_non_exhaustive()
    }
}

impl ApiWorker {
    /// Moves `api` onto a new background thread.
    ///
    /// # Errors
    /// The operating system error when the thread cannot be started.
    pub fn spawn(api: Api) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel::<Job>();
        let thread = std::thread::Builder::new()
            .name("ledgerly-api".to_owned())
            .spawn(move || {
                let mut api = api;
                while let Ok(job) = rx.recv() {
                    job(&mut api);
                }
            })?;
        Ok(Self {
            jobs: Some(tx),
            thread: Some(thread),
        })
    }

    /// Makes the worker wait: every job queued after this call stays queued until the returned
    /// [`Hold`] is dropped. The capture tool uses it to keep a screen in its "computing" state
    /// long enough to photograph; nothing in the app calls it.
    #[must_use]
    pub fn hold(&self) -> Hold {
        let (release, wait) = mpsc::channel::<()>();
        let blocked: Job = Box::new(move |_api: &mut Api| {
            // The sender is dropped when the hold ends, which ends the wait with an error; that
            // is the signal, there is no value to read.
            let _released = wait.recv();
        });
        if let Some(jobs) = &self.jobs {
            // A send error means the thread has stopped; there is nothing left to hold.
            let _stopped = jobs.send(blocked);
        }
        Hold { _release: release }
    }

    /// Queues `job` and returns a ticket for its result. Never blocks.
    ///
    /// When the worker has stopped, the ticket reports [`Poll::Lost`].
    pub fn run<R, F>(&self, job: F) -> Ticket<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Api) -> R + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<R>();
        let boxed: Job = Box::new(move |api: &mut Api| {
            // The screen may have dropped its ticket; then the result is not wanted.
            let _unwanted = tx.send(job(api));
        });
        if let Some(jobs) = &self.jobs {
            // A send error means the thread has stopped; the ticket then reports `Lost`.
            let _stopped = jobs.send(boxed);
        }
        Ticket { rx, done: false }
    }
}

impl Drop for ApiWorker {
    fn drop(&mut self) {
        // Closing the channel ends the loop after the queued jobs have run.
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            // A panicked worker has nothing left to clean up.
            let _joined = thread.join();
        }
    }
}

/// A hold on the worker (see [`ApiWorker::hold`]). Dropping it lets the queued jobs run.
#[derive(Debug)]
pub struct Hold {
    /// Dropped with the hold; the worker's wait ends when this side closes.
    _release: Sender<()>,
}

/// The pending result of one job.
#[derive(Debug)]
pub struct Ticket<R> {
    /// Receives the job's result.
    rx: Receiver<R>,
    /// The result was already taken.
    done: bool,
}

/// What [`Ticket::poll`] found.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll<R> {
    /// The job has not finished.
    Pending,
    /// The job's result. Returned once; later polls give [`Poll::Taken`].
    Ready(R),
    /// The result was already returned by an earlier poll.
    Taken,
    /// The worker stopped before the job ran, so no result will come. Show an error; never a
    /// figure.
    Lost,
}

impl<R> Ticket<R> {
    /// Checks for the result without blocking.
    pub fn poll(&mut self) -> Poll<R> {
        if self.done {
            return Poll::Taken;
        }
        match self.rx.try_recv() {
            Ok(result) => {
                self.done = true;
                Poll::Ready(result)
            }
            Err(TryRecvError::Empty) => Poll::Pending,
            Err(TryRecvError::Disconnected) => Poll::Lost,
        }
    }
}
