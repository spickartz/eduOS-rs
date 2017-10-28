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

use core::sync::atomic::{AtomicUsize, Ordering};
use scheduler::task::*;
use logging::*;
use alloc::VecDeque;
use alloc::boxed::Box;
use alloc::btree_map::*;

static TID_COUNTER: AtomicUsize = AtomicUsize::new(0);

extern {
    pub fn switch(old_stack: *mut u64, new_stack: u64);

	/// The boot loader initialize a stack, which is later also required to
	/// to boot other core. Consequently, the kernel has to replace with this
	/// function the boot stack by a new one.
	pub fn replace_boot_stack(stack_bottom: usize);
}

#[derive(Debug)]
pub enum SchedulerError {
    InvalidReference
}

#[derive(Debug)]
pub struct Scheduler {
	current_task: TaskId,
	ready_queue: Option<VecDeque<TaskId>>,
	finished_tasks: Option<VecDeque<TaskId>>,
	tasks: Option<BTreeMap<TaskId, Box<Task>>>
}

impl Scheduler {
	pub const fn new() -> Scheduler {
		Scheduler {
			current_task: TaskId::from(0),
			ready_queue: None,
			finished_tasks: None,
			tasks: None
		}
	}

	fn get_tid(&self) -> TaskId {
		loop {
			let id = TaskId::from(TID_COUNTER.fetch_add(1, Ordering::SeqCst));

			if self.tasks.as_ref().unwrap().contains_key(&id) == false {
				return id;
			}
		}
	}

	pub fn add_idle_task(&mut self) {
		// idle task is the first task for the scheduler => initialize queue and btree
		self.ready_queue = Some(VecDeque::new());
		self.finished_tasks = Some(VecDeque::new());
		self.tasks = Some(BTreeMap::new());

		// boot task is implicitly task 0 and and the idle task of core 0
		let idle_task = Box::new(Task::new(self.get_tid(), TaskStatus::TaskIdle));

		unsafe {
			// replace temporary boot stack by the kernel stack of the boot task
			replace_boot_stack((*idle_task.stack).bottom());
		}

		self.tasks.as_mut().unwrap().insert(idle_task.id, idle_task);
	}

	pub fn spawn(&mut self, func: extern fn()) -> Result<TaskId, SchedulerError> {
		let id = self.get_tid();
		let mut task = Box::new(Task::new(id, TaskStatus::TaskReady));

		task.create_stack_frame(func);

		self.tasks.as_mut().unwrap().insert(id, task);
		self.ready_queue.as_mut().unwrap().push_back(id);

		info!("create task with id {}", id);

		return Ok(id);
	}

	pub fn exit(&mut self) {
		match self.tasks.as_mut().unwrap().get_mut(&self.current_task) {
			Some(task) => {
				if task.status != TaskStatus::TaskIdle {
					info!("finish task with id {}", self.current_task);
					task.status = TaskStatus::TaskFinished;
				}
			},
			None => info!("unable to find task with id {}", self.current_task)
		}

		self.reschedule();
	}

	#[inline(always)]
	pub fn get_current_taskid(&self) -> TaskId {
		self.current_task
	}

	pub fn schedule(&mut self) {
		let old_task: TaskId = self.current_task;

		// do we have finished tasks? => drop tasks => deallocate implicitly the stack
		match self.finished_tasks.as_mut().unwrap().pop_front() {
			None => {},
			Some(id) => {
				match self.tasks.as_mut().unwrap().remove(&id) {
					Some(task) => {
						drop(task)
					},
					None => info!("unable to drop task {}", id)
				}
			}
		}

		// do we have a task, which is ready?
		match self.ready_queue.as_mut().unwrap().pop_front() {
			None => {
				match self.tasks.as_mut().unwrap().get(&self.current_task) {
					Some(task) => {
						if task.status != TaskStatus::TaskRunning {
							// current task isn't able to run, no other task available => switch to the idle task
							self.current_task = TaskId::from(0);
						}},
					None => info!("unable to find task with id {}", self.current_task)
				}
			},
			Some(id) => self.current_task = id
		}

		// do we have to switch to a new task?
		if old_task != self.current_task {
				let new_stack_pointer: u64;

				match self.tasks.as_mut().unwrap().get_mut(&self.current_task) {
					Some(task) => {
						if task.status != TaskStatus::TaskIdle {
							task.status = TaskStatus::TaskRunning;
						};
						new_stack_pointer = task.last_stack_pointer
					},
					None => panic!("didn't find task {}", self.current_task)
				}

				match self.tasks.as_mut().unwrap().get_mut(&old_task) {
					Some(task) => {
						if task.status == TaskStatus::TaskRunning {
							task.status = TaskStatus::TaskReady;
							self.ready_queue.as_mut().unwrap().push_back(old_task);
						} else if task.status == TaskStatus::TaskFinished {
							task.status = TaskStatus::TaskInvalid;
							self.finished_tasks.as_mut().unwrap().push_back(old_task);
						}

						debug!("switch task from {} to {}", old_task, self.current_task);
						unsafe {
							switch(&mut task.last_stack_pointer, new_stack_pointer);
						}
					},
					None => panic!("didn't find old task")
				}
		}
	}

	#[inline(always)]
	pub fn reschedule(&mut self) {
		self.schedule();
	}
}
