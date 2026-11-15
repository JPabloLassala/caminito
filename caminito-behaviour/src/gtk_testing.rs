//! What the pages' widget tests share: one thread that GTK is started on, which runs them in
//! turn. GTK can be started on one thread only, and each test has a thread of its own, so a
//! second test that started GTK itself would panic. Where GTK can't start, as with no
//! display, the tests are skipped.

use std::panic::{self, UnwindSafe};
use std::sync::{Mutex, OnceLock, mpsc};

/// A test, given whether GTK started.
type Job = Box<dyn FnOnce(bool) + Send>;

/// Runs `test` on GTK's thread, and passes its panic on, or skips it where GTK can't start.
pub(crate) fn on_gtk(test: impl FnOnce() + Send + UnwindSafe + 'static) {
    static GTK_THREAD: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
    let jobs = GTK_THREAD.get_or_init(|| {
        let (jobs, queue) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            let started = gtk::init().is_ok();
            for job in queue {
                job(started);
            }
        });
        Mutex::new(jobs)
    });
    let (done, outcome) = mpsc::channel();
    let job: Job = Box::new(move |started| {
        let result = if started {
            panic::catch_unwind(test)
        } else {
            Ok(())
        };
        let _ = done.send(result);
    });
    jobs.lock()
        .expect("no test panics while it holds the queue")
        .send(job)
        .expect("GTK's thread runs as long as the tests");
    if let Err(panic) = outcome.recv().expect("GTK's thread answers each test") {
        panic::resume_unwind(panic);
    }
}
