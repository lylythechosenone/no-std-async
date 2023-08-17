use crate::{
    mutex::{Guard, Mutex},
    rwlock::{ReadGuard, RwLock},
};

/// A reader-writer lock that only syncs when the `sync` method is called.
///
/// This is similar to a [`RwLock`], except that writes are not instantly
/// realized by parallel readers. Instead, writes are only realized when the
/// `sync` method is called. This is useful for cases where you want to write to a value, but you don't
/// want to block readers while you do so.
///
/// This type only implements [`Sync`] when `T` is [`Send`]. Syncing and the non-const [`new`](Self::new)
/// method require `T` to implement [`Clone`]. Values are provided through the [`Deref`](std::ops::Deref)
/// and [`DerefMut`](std::ops::DerefMut) implementations on the [`ReadGuard`] and [`Guard`] types.
///
/// # Examples
/// ```rust
/// use no_std_async::SwapLock;
/// # fn main() { pollster::block_on(foo()); }
///
/// async fn foo() {
///     let lock = SwapLock::new(42);
///     let read = lock.read().await;
///     assert_eq!(*read, 42);
///
///     let mut write = lock.write().await;
///     *write += 1;
///     assert_eq!(*write, 43); // All writers will read the new value.
///     assert_eq!(*read, 42); // The read value is not updated until `sync` is called.
///
///     drop(read);
///     drop(write);
///
///     lock.sync().await;
///     let read = lock.read().await;
///     assert_eq!(*read, 43); // The value has now been updated.
/// }
pub struct SwapLock<T> {
    data: RwLock<T>,
    write: Mutex<T>,
}
impl<T> SwapLock<T> {
    /// Creates a new [`SwapLock`] with the given data.
    pub fn new(val: T) -> Self
    where
        T: Clone,
    {
        Self {
            data: RwLock::new(val.clone()),
            write: Mutex::new(val),
        }
    }
    /// Creates a new [`SwapLock`] with the given data and write value.
    /// These values should be the same. In [`new`](Self::new), it is simply cloned,
    /// but this is not possible in a const context.
    pub const fn const_new(data: T, write: T) -> Self {
        Self {
            data: RwLock::new(data),
            write: Mutex::new(write),
        }
    }

    /// Syncs the data with the written value.
    /// This function waits for ***all locks*** to be released.
    /// This means that holding a read ***or*** write lock will deadlock
    /// if you call this function.
    pub async fn sync(&self)
    where
        T: Clone,
    {
        *self.data.write().await = self.write.lock().await.clone();
    }

    /// Acquires a read lock on the data.
    pub async fn read(&self) -> ReadGuard<'_, T> {
        self.data.read().await
    }
    /// Acquires a write lock on the data.
    pub async fn write(&self) -> Guard<'_, T> {
        self.write.lock().await
    }
}

unsafe impl<T: Send> Send for SwapLock<T> {}
unsafe impl<T: Send> Sync for SwapLock<T> {}
