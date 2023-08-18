use spin::Mutex;

use crate::Semaphore;

/// A condition variable.
///
/// This type allows multiple tasks to wait for an action to be completed.
/// This type is similar to [`std::sync::Condvar`], except that it is
/// async and cross-platform, and cannot be spuriously woken.
///
/// # Examples
/// ```rust
/// use no_std_async::{Condvar, Mutex};
/// # fn main() { std::thread::spawn(|| pollster::block_on(task1())); pollster::block_on(task2()); }
///
/// static CONDVAR: Condvar = Condvar::new();
/// static VALUE: Mutex<u8> = Mutex::new(0);
///
/// async fn task1() {
///     for i in 0..10 {
///         *VALUE.lock().await += 1; // some work
///     }
///     CONDVAR.notify_one(); // we're done!
/// }
///
/// async fn task2() {
///     CONDVAR.wait().await; // wait for task1 to finish
///     assert_eq!(10, *VALUE.lock().await);
/// }
/// ```
pub struct Condvar {
    semaphore: Semaphore,
    num_waiters: Mutex<usize>,
}
impl Condvar {
    /// Creates a new [`Condvar`].
    pub const fn new() -> Self {
        Self {
            semaphore: Semaphore::new(0),
            num_waiters: Mutex::new(0),
        }
    }

    /// Blocks the current task until this condition variable receives a notification.
    pub async fn wait(&self) {
        let mut num_waiters = self.num_waiters.lock();
        *num_waiters += 1;
        drop(num_waiters);
        self.semaphore.acquire(1).await;
    }

    /// Notifies a single task waiting on this condition variable.
    pub fn notify_one(&self) {
        let mut num_waiters = self.num_waiters.lock();
        if *num_waiters > 0 {
            *num_waiters -= 1;
            self.semaphore.release(1);
        }
    }

    /// Notifies all tasks waiting on this condition variable.
    pub fn notify_all(&self) {
        let mut num_waiters = self.num_waiters.lock();
        if *num_waiters > 0 {
            let total_waiters = *num_waiters;
            *num_waiters = 0;
            self.semaphore.release(total_waiters);
        }
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::thread;

    use super::*;

    // doctests test `notify_one` for us

    #[test]
    fn notify_all() {
        static CONDVAR: Condvar = Condvar::new();

        let task1 = thread::spawn(|| pollster::block_on(CONDVAR.wait()));
        let task2 = thread::spawn(|| pollster::block_on(CONDVAR.wait()));
        let task3 = thread::spawn(|| pollster::block_on(CONDVAR.wait()));

        thread::sleep(Duration::from_millis(100)); // make sure all tasks are waiting

        CONDVAR.notify_all();

        thread::sleep(Duration::from_millis(100)); // time for everything to synchronize

        assert!(task1.is_finished());
        assert!(task2.is_finished());
        assert!(task3.is_finished());
    }
}
