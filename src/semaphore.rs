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
    Unprotected = (),
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
        if let Ok(waker) = lock.waiters.cursor_front_mut().remove_current(()) {
            drop(lock);
            waker.wake();
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

        if projected.node.is_initial() {
            lock.waiters
                .cursor_back_mut()
                .insert_after(projected.node, cx.waker().clone(), ());
        } else {
            // We've been woken, but not enough is ready for us yet. Keep waiting.
            lock.waiters
                .cursor_front_mut()
                .insert_before(projected.node, cx.waker().clone(), ());
        }

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
            (pin_list::NodeData::Linked(_waker), ()) => {} // We've been cancelled before ever being woken.
            (pin_list::NodeData::Removed(()), ()) => {
                // Oops, we were already woken! We need to wake the next task in line.
                if let Ok(waker) = lock.waiters.cursor_front_mut().remove_current(()) {
                    drop(lock);
                    waker.wake();
                }
            }
        }
    }
}
