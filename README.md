# no-std-async
##### #[no_std] async synchronization primitives for rust

## Types
This crate provides a few synchronization primitives:
- [`Mutex`](https://docs.rs/no-std-async/latest/no_std_async/mutex/struct.Mutex.html)
- [`RwLock`](https://docs.rs/no-std-async/latest/no_std_async/rwlock/struct.RwLock.html)
- [`Semaphore`](https://docs.rs/no-std-async/latest/no_std_async/semaphore/struct.Semaphore.html)
- [`SwapLock`](https://docs.rs/no-std-async/latest/no_std_async/swaplock/struct.SwapLock.html)

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
no-std-async = "version"
```
where `version` is the latest crate version.

The various types in this crate provide specific usage examples.
