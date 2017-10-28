// Copyright (c) 2017 Stefan Lankes, RWTH Aachen University
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use alloc::VecDeque;
use scheduler::task::TaskId;
use scheduler::{get_current_taskid,reschedule,block_current_task, wakeup_task};
use synch::spinlock::*;

/// A counting, blocking, semaphore.
///
/// Semaphores are a form of atomic counter where access is only granted if the
/// counter is a positive value. Each acquisition will block the calling thread
/// until the counter is positive, and each release will increment the counter
/// and unblock any threads if necessary.
///
/// # Examples
///
/// ```
///
/// // Create a semaphore that represents 5 resources
/// let sem = Semaphore::new(5);
///
/// // Acquire one of the resources
/// sem.acquire();
///
/// // Acquire one of the resources for a limited period of time
/// {
///     let _guard = sem.access();
///     // ...
/// } // resources is released here
///
/// // Release our initially acquired resource
/// sem.release();
///
/// Interface is derived from https://doc.rust-lang.org/1.7.0/src/std/sync/semaphore.rs.html
/// ```
pub struct Semaphore {
	/// Resource available count
	value: SpinlockIrqSave<isize>,
	/// Queue of waiting tasks
	queue: SpinlockIrqSave<VecDeque<TaskId>>,
}

/// An RAII guard which will release a resource acquired from a semaphore when
/// dropped.
pub struct SemaphoreGuard<'a> {
    sem: &'a Semaphore,
}

impl Semaphore {
	/// Creates a new semaphore with the initial count specified.
	///
	/// The count specified can be thought of as a number of resources, and a
	/// call to `acquire` or `access` will block until at least one resource is
	/// available. It is valid to initialize a semaphore with a negative count.
	pub fn new(count: isize) -> Semaphore {
        Semaphore {
			value: SpinlockIrqSave::new(count),
			queue: SpinlockIrqSave::new(VecDeque::new())
        }
	}

	/// Acquires a resource of this semaphore, blocking the current thread until
    /// it can do so.
    ///
    /// This method will block until the internal count of the semaphore is at
    /// least 1.
    pub fn acquire(&self) {
		let mut done: bool = false;

		while done == false {
			let mut count = self.value.lock();

    		if *count > 0 {
        		*count -= 1;
				done = true;
    		} else {
				self.queue.lock().push_back(get_current_taskid());
				block_current_task();
				// release lock
				drop(count);
				// switch to the next task
				reschedule();
			}
		}
    }

	/// Release a resource from this semaphore.
    ///
    /// This will increment the number of resources in this semaphore by 1 and
    /// will notify any pending waiters in `acquire` or `access` if necessary.
    pub fn release(&self) {
		let mut count = self.value.lock();
		*count += 1;

		// try to wakeup next task
		match self.queue.lock().pop_front() {
			None => {}
			Some(id) => wakeup_task(id)
		}
    }

    /// Acquires a resource of this semaphore, returning an RAII guard to
    /// release the semaphore when dropped.
    ///
    /// This function is semantically equivalent to an `acquire` followed by a
    /// `release` when the guard returned is dropped.
    pub fn access(&mut self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { sem: self }
    }
}

impl<'a> Drop for SemaphoreGuard<'a>
{
    /// The dropping of the SemaphoreGuard will release the lock it was created from.
    fn drop(&mut self)
    {
		self.sem.release();
    }
}