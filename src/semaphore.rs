use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use pin_list::PinList;
use pin_project::{pin_project, pinned_drop};
use spin::Mutex;

type PinListTypes = dyn pin_list::Types<
    Id = pin_list::id::Unchecked,
    Protected = Waker,
    Removed = (),
    Unprotected = usize,
>;

/// A type that asynchronously distributes "permits."
///
/// A permit is a token that allows the holder to perform some action.
/// The semaphore itself does not dictate this action, but instead
/// handles only the distribution of permits.
/// This is useful for cases where you want to limit the number of
/// concurrent operations, such as in a mutex.
///
/// `n` permits can be acquired through [`acquire`](Self::acquire).
/// They can later be released through [`release`](Self::release).
///
/// # Examples
/// For examples, look at the implementations of [`Mutex`](crate::Mutex) and [`RwLock`](crate::RwLock).
/// [`Mutex`](crate::Mutex) uses a semaphore with a maximum of 1 permit to allow a single lock at a time.
/// [`RwLock`](crate::RwLock) uses a semaphore with a maximum of `max_readers` permits to allow any number of readers.
/// When a `write` call is encountered, it acquires all of the permits, blocking any new readers from locking.
pub struct Semaphore {
    inner: Mutex<SemaphoreInner>,
}
impl Semaphore {
    /// Creates a new [`Semaphore`] with the given initial number of permits.
    pub const fn new(initial_count: usize) -> Self {
        Self {
            inner: Mutex::new(SemaphoreInner {
                count: initial_count,
                waiters: PinList::new(unsafe { pin_list::id::Unchecked::new() }),
            }),
        }
    }

    /// Acquires `n` permits from the semaphore.
    /// These permits should be [`release`](Self::release)d later,
    /// or they will be permanently removed.
    pub fn acquire(&self, n: usize) -> Acquire<'_> {
        #[cfg(test)]
        println!("acquire({})", n);
        Acquire {
            semaphore: self,
            n,
            node: pin_list::Node::new(),
        }
    }

    /// Releases `n` permits back to the semaphore.
    pub fn release(&self, n: usize) {
        let mut lock = self.inner.lock();
        lock.count += n;
        match lock.waiters.cursor_front_mut().unprotected().copied() {
            Some(count) if lock.count >= count => {
                let waker = lock.waiters.cursor_front_mut().remove_current(()).unwrap();
                drop(lock);
                waker.wake();
            }
            _ => {}
        }
    }

    /// Returns the number of remaining permits.
    pub fn remaining(&self) -> usize {
        self.inner.lock().count
    }
}

struct SemaphoreInner {
    count: usize,
    waiters: PinList<PinListTypes>,
}

/// A future that acquires a permit from a [`Semaphore`].
/// This future should not be dropped before completion,
/// otherwise the permit will not be acquired.
#[must_use]
#[pin_project(PinnedDrop)]
pub struct Acquire<'a> {
    semaphore: &'a Semaphore,
    n: usize,
    #[pin]
    node: pin_list::Node<PinListTypes>,
}
impl Future for Acquire<'_> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut projected = self.project();

        let mut lock = projected.semaphore.inner.lock();

        if let Some(node) = projected.node.as_mut().initialized_mut() {
            if let Err(e) = node.take_removed(&lock.waiters) {
                // Someone has polled us again, but we haven't been woken yet.
                // We update the waker, then go back to sleep.
                *e.protected_mut(&mut projected.semaphore.inner.lock().waiters)
                    .unwrap() = cx.waker().clone();
                return Poll::Pending;
            }
        }

        if lock.count >= *projected.n {
            lock.count -= *projected.n;
            if lock.count > 0 {
                // There's still more for others to take, give it to the next task in line.
                if let Ok(waker) = lock.waiters.cursor_front_mut().remove_current(()) {
                    drop(lock);
                    waker.wake();
                }
            }
            return Poll::Ready(());
        }

        lock.waiters.cursor_back_mut().insert_after(
            projected.node,
            cx.waker().clone(),
            *projected.n,
        );

        Poll::Pending
    }
}
#[pinned_drop]
impl PinnedDrop for Acquire<'_> {
    fn drop(self: Pin<&mut Self>) {
        let projected = self.project();
        let node = match projected.node.initialized_mut() {
            Some(node) => node,
            None => return, // We're either already done or never started. In either case, we can just return.
        };

        let mut lock = projected.semaphore.inner.lock();

        match node.reset(&mut lock.waiters) {
            (pin_list::NodeData::Linked(_waker), _) => {} // We've been cancelled before ever being woken.
            (pin_list::NodeData::Removed(()), _) => {
                // Oops, we were already woken! We need to wake the next task in line.
                if let Ok(waker) = lock.waiters.cursor_front_mut().remove_current(()) {
                    drop(lock);
                    waker.wake();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn semaphore() {
        static SEMAPHORE: Semaphore = Semaphore::new(10);

        let take_10 = thread::spawn(|| pollster::block_on(SEMAPHORE.acquire(10))); // should complete instantly
        thread::sleep(std::time::Duration::from_millis(10));
        assert!(take_10.is_finished());

        let take_1 = thread::spawn(|| pollster::block_on(SEMAPHORE.acquire(1)));
        thread::sleep(std::time::Duration::from_millis(10));
        let take_30 = thread::spawn(|| pollster::block_on(SEMAPHORE.acquire(30)));
        thread::sleep(std::time::Duration::from_millis(10));
        let take_5 = thread::spawn(|| pollster::block_on(SEMAPHORE.acquire(5)));
        thread::sleep(std::time::Duration::from_millis(10));

        SEMAPHORE.release(30);
        thread::sleep(std::time::Duration::from_millis(10));
        assert!(take_1.is_finished());
        assert!(!take_30.is_finished()); // we only have 29 now
        assert!(!take_5.is_finished()); // take_30 waits at the start of the line and doesn't notify 5

        SEMAPHORE.release(6);
        thread::sleep(std::time::Duration::from_millis(10));
        assert!(take_30.is_finished());
        assert!(take_5.is_finished());
    }
}
