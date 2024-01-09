use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

use crate::semaphore::Semaphore;

/// An async reader-writer lock.
///
/// This lock allows `max_readers` readers at once, or one writer. This is different from a mutex,
/// which only allows one *lock* at a time, regardless of whether it is used for reading or writing.
///
/// This lock is fair, meaning that locks are provided in the order that they are requested.
/// This is in contrast to a read-preferring lock, which would continue to allow readers to lock
/// even if a writer is waiting. This stops the possibility of readers starving writers.
///
/// This type only implements [`Sync`] when `T` is [`Send`]. Values are provided through the [`Deref`]
/// and [`DerefMut`] implementations on the [`ReadGuard`] and [`WriteGuard`] types.
///
/// # Examples
/// ```rust
/// use no_std_async::RwLock;
/// # fn main() { pollster::block_on(foo()); }
///
/// async fn foo() {
///     let lock = RwLock::new(42);
///
///     {
///         // we can have as many readers as we want.
///         let reads = [
///            lock.read().await,
///            lock.read().await,
///            lock.read().await,
///            lock.read().await,
///            // ...and so on
///         ];
///         assert!(reads.iter().all(|r| **r == 42));
///     } // all readers are dropped here.
///
///     {
///         // we can only have one writer at a time.
///         let mut write = lock.write().await;
///         *write += 1;
///         assert_eq!(*write, 43);
///     } // the writer is dropped here.
/// }
/// ```
pub struct RwLock<T> {
    data: UnsafeCell<T>,
    semaphore: Semaphore,
    max_readers: usize,
}
impl<T> RwLock<T> {
    /// Creates a new [`RwLock`] with the given data.
    pub const fn new(data: T) -> Self {
        Self::with_max_readers(data, usize::MAX)
    }
    /// Creates a new [`RwLock`] with the given data and maximum number of readers.
    pub const fn with_max_readers(data: T, max_readers: usize) -> Self {
        Self {
            data: UnsafeCell::new(data),
            semaphore: Semaphore::new(max_readers),
            max_readers,
        }
    }

    /// Acquires a read lock on the data.
    pub async fn read(&self) -> ReadGuard<'_, T> {
        self.semaphore.acquire(1).await;
        ReadGuard { rwlock: self }
    }
    /// Acquires a write lock on the data.
    pub async fn write(&self) -> WriteGuard<'_, T> {
        self.semaphore.acquire(self.max_readers).await;
        WriteGuard { rwlock: self }
    }
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send> Sync for RwLock<T> {}

/// A guard that provides immutable access to the data in an [`RwLock`].
/// The data can be accessed using the [`Deref`] implementation.
pub struct ReadGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}
impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}
impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.semaphore.release(1);
    }
}

/// A guard that provides mutable access to the data in an [`RwLock`].
/// The data can be accessed using the [`Deref`] and [`DerefMut`] implementations.
pub struct WriteGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}
impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}
impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.data.get() }
    }
}
impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.semaphore.release(self.rwlock.max_readers);
    }
}
