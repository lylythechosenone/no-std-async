use crate::semaphore::Semaphore;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

/// An async mutex.
/// This is similar to the [`Mutex`] type in the standard library, but it is async.
///
/// This type only implements [`Sync`] when `T` is [`Send`]. Values are provided through the [`Deref`]
/// and [`DerefMut`] implementations on the [`Guard`] type.
///
/// # Examples
/// ```rust
/// use no_std_async::Mutex;
/// # fn main() { std::thread::spawn(|| pollster::block_on(task_1())); pollster::block_on(task_2()); }
///
/// static COUNT: Mutex<u8> = Mutex::new(0);
///
/// // These functions will each trade the lock, counting it up to infinity (in this case, u8::MAX).
///
/// async fn task_1() {
///     loop {
///         let mut count = COUNT.lock().await;
///         if *count == u8::MAX {
///             break;
///         }
///         let expected = *count + 1;
///         *count += 1;
///         assert_eq!(expected, *count); // we have the lock, so our value can't be modified otherwise.
///     }
/// }
///
/// async fn task_2() {
///    loop {
///       let mut count = COUNT.lock().await;
///       if *count == u8::MAX {
///           break;
///       }
///       let expected = *count + 1;
///       *count += 1;
///       assert_eq!(expected, *count); // we have the lock, so our value can't be modified otherwise.
///   }
/// }
///
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    semaphore: Semaphore,
}
impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] with the given data.
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            semaphore: Semaphore::new(1),
        }
    }

    /// Acquires a lock on the data.
    pub async fn lock<'a>(&'a self) -> Guard<'a, T>
    where
        T: 'a,
    {
        self.semaphore.acquire(1).await;
        Guard { mutex: self }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

/// A guard that provides mutable access to the data inside of a [`Mutex`].
/// The data can be accessed using the [`Deref`] and [`DerefMut`] implementations.
pub struct Guard<'a, T> {
    mutex: &'a Mutex<T>,
}
impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}
impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.mutex.semaphore.release(1);
    }
}
