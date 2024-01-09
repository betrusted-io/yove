// use parking_lot::{lock_api::RawMutex as RawMutexTrait, RawMutex};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::AtomicUsize,
        mpsc::{channel, Sender},
        Arc, Condvar, Mutex,
    },
    thread,
};

use super::ScalarResult;
use crate::xous::{definitions::SyscallResultNumber, Memory};

type CondvarIndex = Arc<(Condvar, AtomicUsize)>;

pub struct Ticktimer {
    start: std::time::SystemTime,
    condvars: Arc<Mutex<HashMap<usize, CondvarIndex>>>,
    mutexes: Arc<Mutex<HashMap<u32, bool>>>,
    mutex_unlockers: Arc<Mutex<HashMap<u32, VecDeque<Sender<()>>>>>,
}

enum ScalarOpcode {
    ElapsedMs = 0,
    LockMutex = 6,
    UnlockMutex = 7,
    FreeMutex = 10,
    WaitForCondition = 8,
    NotifyCondition = 9,
    FreeCondition = 11,
}

impl Ticktimer {
    pub fn new() -> Self {
        // eprintln!("Created new Ticktimer");
        Ticktimer {
            start: std::time::SystemTime::now(),
            condvars: Arc::new(Mutex::new(HashMap::new())),
            mutexes: Arc::new(Mutex::new(HashMap::new())),
            mutex_unlockers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn lock_mutex(&self, mutex_index: u32) -> ScalarResult {
        // eprintln!("Locking mutex {:08x}", mutex_index);
        let mut mutexes = self.mutexes.lock().unwrap();
        let mutex_locked = mutexes.entry(mutex_index).or_default();
        if *mutex_locked {
            let (wakeup_tx, wakeup_rx) = channel();
            // Mutex was locked by a different thread. Pause this thread until it is unlocked.
            let (tx, rx) = channel();
            thread::spawn(move || {
                wakeup_rx.recv().unwrap();
                tx.send((
                    [SyscallResultNumber::Scalar1 as i32, 0, 0, 0, 0, 0, 0, 0],
                    None,
                ))
                .unwrap();
            });
            self.mutex_unlockers
                .lock()
                .unwrap()
                .entry(mutex_index)
                .or_default()
                .push_back(wakeup_tx);
            return ScalarResult::WaitForResponse(rx);
        }
        *mutex_locked = true;
        ScalarResult::Scalar1(0)
    }

    fn unlock_mutex(&self, mutex_index: u32) -> ScalarResult {
        // eprintln!("Unlocking mutex {:08x}", mutex_index);
        let mut mutexes = self.mutexes.lock().unwrap();
        let mutex_locked = mutexes.get_mut(&mutex_index).expect("mutex didn't exist");
        assert!(*mutex_locked);
        *mutex_locked = false;

        // Wake up one waiter if one existed
        if let Some(Some(unlocker)) = self
            .mutex_unlockers
            .lock()
            .unwrap()
            .get_mut(&mutex_index)
            .map(|v| v.pop_front())
        {
            unlocker.send(()).unwrap();
        }
        ScalarResult::Scalar1(0)
    }

    fn free_mutex(&self, mutex_index: u32) -> ScalarResult {
        // eprintln!("Freeing mutex {:08x}", mutex_index);
        self.mutexes
            .lock()
            .unwrap()
            .remove(&mutex_index)
            .expect("mutex didn't exist");
        ScalarResult::Scalar1(0)
    }

    fn wait_for_condition(&self, condition_index: usize, wait_count: u64) -> ScalarResult {
        let (tx, rx) = channel();
        let condvar = self
            .condvars
            .lock()
            .unwrap()
            .entry(condition_index)
            .or_insert_with(|| Arc::new((Condvar::new(), AtomicUsize::new(0))))
            .clone();
        // println!(
        //     "Waiting for condition {:08x} with a count of {} ms",
        //     condition_index, wait_count
        // );
        condvar.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        thread::spawn(move || {
            let dummy_mutex = Mutex::new(());
            let guard = dummy_mutex.lock().unwrap();
            let timeout_value = if wait_count == 0 {
                let _ignored = condvar.0.wait(guard).unwrap();
                0
            } else if condvar
                .0
                .wait_timeout(guard, std::time::Duration::from_millis(wait_count))
                .unwrap()
                .1
                .timed_out()
            {
                1
            } else {
                0
            };
            condvar.1.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            tx.send((
                [
                    super::super::definitions::SyscallResultNumber::Scalar1 as i32,
                    timeout_value,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
                None,
            ))
            .unwrap();
        });
        super::ScalarResult::WaitForResponse(rx)
    }

    fn notify_condition(&self, condition_index: usize, condition_count: usize) -> ScalarResult {
        // println!(
        //     "Notifying condition {:08x} {} times",
        //     condition_index, condition_count
        // );
        if condition_count == 0 || !self.condvars.lock().unwrap().contains_key(&condition_index) {
            return super::ScalarResult::Scalar5([0, 0, 0, 0, 0]);
        }
        let mut notify_count = 0;
        if let Some(condvar) = self.condvars.lock().unwrap().get(&condition_index) {
            if condition_count == 0 {
                notify_count = condvar.1.load(std::sync::atomic::Ordering::Relaxed);
                condvar.0.notify_all();
            } else {
                for _ in 0..condition_count {
                    notify_count += 1;
                    condvar.0.notify_one();
                }
            }
        }
        super::ScalarResult::Scalar1(notify_count as u32)
    }
}

impl Default for Ticktimer {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Service for Ticktimer {
    fn scalar(&self, _memory: &Memory, _sender: u32, opcode: u32, args: [u32; 4]) {
        if opcode == ScalarOpcode::FreeCondition as u32 {
            let condition_index = args[0] as usize;
            if let Some(condvar) = self.condvars.lock().unwrap().remove(&condition_index) {
                assert!(condvar.1.load(std::sync::atomic::Ordering::Relaxed) == 0);
            }
        } else {
            println!("Unhandled scalar: {}", opcode);
        }
    }

    fn blocking_scalar(
        &self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        args: [u32; 4],
    ) -> super::ScalarResult {
        if opcode == ScalarOpcode::ElapsedMs as u32 {
            let elapsed_ms = std::time::SystemTime::now()
                .duration_since(self.start)
                .unwrap()
                .as_millis() as u64;
            super::ScalarResult::Scalar2([elapsed_ms as u32, (elapsed_ms >> 32) as u32])
        } else if opcode == ScalarOpcode::LockMutex as u32 {
            self.lock_mutex(args[0])
        } else if opcode == ScalarOpcode::UnlockMutex as u32 {
            self.unlock_mutex(args[0])
        } else if opcode == ScalarOpcode::FreeMutex as u32 {
            self.free_mutex(args[0])
        } else if opcode == ScalarOpcode::WaitForCondition as u32 {
            self.wait_for_condition(args[0] as usize, args[1] as u64)
        } else if opcode == ScalarOpcode::NotifyCondition as u32 {
            self.notify_condition(args[0] as usize, args[1] as usize)
        } else {
            panic!(
                "Ticktimer unhandled blocking_scalar {}: {} {:x?}",
                sender, opcode, args
            );
        }
    }
}
