use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, Condvar, Mutex},
    thread,
};

use super::LendResult;

pub struct Ticktimer {
    start: std::time::SystemTime,
    condvars: HashMap<usize, Arc<(Condvar, AtomicUsize)>>,
}

enum ScalarOpcode {
    ElapsedMs = 0,
    WaitForCondition = 8,
    NotifyCondition = 9,
    FreeCondition = 11,
}

impl Ticktimer {
    pub fn new() -> Self {
        // println!("Constructing a ticktimer");
        Ticktimer {
            start: std::time::SystemTime::now(),
            condvars: HashMap::new(),
        }
    }
}

impl Default for Ticktimer {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Service for Ticktimer {
    fn scalar(&mut self, _sender: u32, opcode: u32, args: [u32; 4]) {
        if opcode == ScalarOpcode::FreeCondition as u32 {
            let condition_index = args[0] as usize;
            if let Some(condvar) = self.condvars.remove(&condition_index) {
                assert!(condvar.1.load(std::sync::atomic::Ordering::Relaxed) == 0);
            }
        }
    }

    fn blocking_scalar(&mut self, sender: u32, opcode: u32, args: [u32; 4]) -> super::ScalarResult {
        if opcode == ScalarOpcode::ElapsedMs as u32 {
            let elapsed_ms = std::time::SystemTime::now()
                .duration_since(self.start)
                .unwrap()
                .as_millis() as u64;
            super::ScalarResult::Scalar2([elapsed_ms as u32, (elapsed_ms >> 32) as u32])
        } else if opcode == ScalarOpcode::WaitForCondition as u32 {
            let condition_index = args[0] as usize;
            let wait_count = args[1] as u64;

            let (tx, rx) = std::sync::mpsc::channel();
            let condvar = self
                .condvars
                .entry(condition_index)
                .or_insert_with(|| Arc::new((Condvar::new(), AtomicUsize::new(0))))
                .clone();
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
                        super::super::definitions::SyscallResultNumber::Scalar1 as i64,
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
        } else if opcode == ScalarOpcode::NotifyCondition as u32 {
            let condition_index = args[0] as usize;
            let condition_count = args[1] as usize;
            if condition_count == 0 || !self.condvars.contains_key(&condition_index) {
                return super::ScalarResult::Scalar5([0, 0, 0, 0, 0]);
            }
            let mut notify_count = 0;
            if let Some(condvar) = self.condvars.get(&condition_index) {
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
        } else {
            panic!(
                "Ticktimer blocking_scalar {}: {} {:x?}",
                sender, opcode, args
            );
        }
    }

    fn lend(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) -> LendResult {
        println!("Ticktimer lend {}: {} {:x?}", sender, opcode, extra);
        LendResult::MemoryReturned([0, 0])
    }

    fn lend_mut(
        &mut self,
        sender: u32,
        opcode: u32,
        _buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        println!("Ticktimer lend_mut {}: {} {:x?}", sender, opcode, extra);
        LendResult::MemoryReturned([0, 0])
    }

    fn send(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) {
        println!("Ticktimer send {}: {} {:x?}", sender, opcode, extra);
    }
}
