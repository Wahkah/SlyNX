//! A bounded single-producer, single-consumer queue for passing commands between the game thread and the audio
//! thread without locks or allocation after creation.
//!
//! The ring has `capacity` slots. The producer owns `tail` and the consumer owns `head`; each publishes its index
//! with release ordering and reads the other's with acquire ordering, so a slot's contents are visible before its
//! index is.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

struct Ring<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    /// Count of items taken (consumer side).
    head: AtomicUsize,
    /// Count of items added (producer side).
    tail: AtomicUsize,
}

// SAFETY: a slot is only ever accessed by one side at a time: the producer writes slots in `tail..head + capacity`
// (free) and the consumer reads slots in `head..tail` (full). The indices hand each slot from one side to the other
// with release/acquire ordering. `T: Send` because values move between threads.
unsafe impl<T: Send> Sync for Ring<T> {}
// SAFETY: as above; the ring owns its values.
unsafe impl<T: Send> Send for Ring<T> {}

impl<T> Drop for Ring<T> {
    fn drop(&mut self) {
        let head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        let cap = self.slots.len();
        for i in head..tail {
            // SAFETY: slots in `head..tail` hold initialised values that were never taken; we have `&mut self`.
            unsafe { (*self.slots[i % cap].get()).assume_init_drop() };
        }
    }
}

/// The sending half. Not `Clone`: there is exactly one producer.
pub struct Producer<T> {
    ring: Arc<Ring<T>>,
}

/// The receiving half. Not `Clone`: there is exactly one consumer.
pub struct Consumer<T> {
    ring: Arc<Ring<T>>,
}

/// A queue with room for `capacity` items (at least 1).
pub fn channel<T: Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let capacity = capacity.max(1);
    let slots: Vec<UnsafeCell<MaybeUninit<T>>> =
        (0..capacity).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
    let ring = Arc::new(Ring { slots: slots.into_boxed_slice(), head: AtomicUsize::new(0), tail: AtomicUsize::new(0) });
    (Producer { ring: ring.clone() }, Consumer { ring })
}

impl<T: Send> Producer<T> {
    /// Adds `value`, or gives it back when the queue is full.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let r = &*self.ring;
        let tail = r.tail.load(Ordering::Relaxed);
        let head = r.head.load(Ordering::Acquire);
        let cap = r.slots.len();
        if tail.wrapping_sub(head) >= cap {
            return Err(value);
        }
        // SAFETY: the slot at `tail` is free (the consumer has taken everything before `head`, and
        // `tail - head < cap`), and only this producer writes slots.
        unsafe { (*r.slots[tail % cap].get()).write(value) };
        r.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Free slots right now (the consumer may free more at any time).
    pub fn free(&self) -> usize {
        let r = &*self.ring;
        r.slots.len() - r.tail.load(Ordering::Relaxed).wrapping_sub(r.head.load(Ordering::Acquire))
    }
}

impl<T: Send> Consumer<T> {
    /// Takes the oldest item, if any.
    pub fn pop(&mut self) -> Option<T> {
        let r = &*self.ring;
        let head = r.head.load(Ordering::Relaxed);
        let tail = r.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let cap = r.slots.len();
        // SAFETY: `head != tail`, so the slot at `head` holds a value the producer published (acquire above), and
        // only this consumer reads slots. The value is moved out once; advancing `head` marks the slot free.
        let v = unsafe { (*r.slots[head % cap].get()).assume_init_read() };
        r.head.store(head.wrapping_add(1), Ordering::Release);
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_full() {
        let (mut tx, mut rx) = channel::<u32>(3);
        assert_eq!(tx.free(), 3);
        tx.push(1).unwrap();
        tx.push(2).unwrap();
        tx.push(3).unwrap();
        assert_eq!(tx.push(4), Err(4));
        assert_eq!(rx.pop(), Some(1));
        tx.push(4).unwrap();
        assert_eq!(rx.pop(), Some(2));
        assert_eq!(rx.pop(), Some(3));
        assert_eq!(rx.pop(), Some(4));
        assert_eq!(rx.pop(), None);
        for i in 0..100 {
            tx.push(i).unwrap();
            assert_eq!(rx.pop(), Some(i));
        }
    }

    #[test]
    fn drops_leftovers() {
        let marker = Arc::new(());
        {
            let (mut tx, _rx) = channel::<Arc<()>>(4);
            tx.push(marker.clone()).unwrap();
            tx.push(marker.clone()).unwrap();
            assert_eq!(Arc::strong_count(&marker), 3);
        }
        assert_eq!(Arc::strong_count(&marker), 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn across_threads() {
        extern crate std;
        let (mut tx, mut rx) = channel::<u64>(16);
        let t = std::thread::spawn(move || {
            let mut i = 0u64;
            while i < 100_000 {
                if tx.push(i).is_ok() {
                    i += 1;
                }
            }
        });
        let mut expect = 0u64;
        while expect < 100_000 {
            if let Some(v) = rx.pop() {
                assert_eq!(v, expect);
                expect += 1;
            }
        }
        t.join().unwrap();
    }
}
