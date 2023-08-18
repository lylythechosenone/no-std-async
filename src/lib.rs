#![cfg_attr(not(any(doctest, test)), no_std)]
#![doc = include_str!("../README.md")]

/// Auxiliary types for the `Condvar` type
pub mod condvar;
/// Auxiliary types for the `Mutex` type
pub mod mutex;
/// Auxiliary types for the `RwLock` type
pub mod rwlock;
/// Auxiliary types for the `Semaphore` type
pub mod semaphore;
/// Auxiliary types for the `SwapLock` type
pub mod swap_lock;

pub use condvar::Condvar;
pub use mutex::Mutex;
pub use rwlock::RwLock;
pub use semaphore::Semaphore;
pub use swap_lock::SwapLock;
