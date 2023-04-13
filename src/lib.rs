use std::time::{SystemTime, UNIX_EPOCH};

pub mod atomic;
pub mod pooled;

/// The `SnowflakeIdGen` type is snowflake algorithm wrapper.
#[derive(Copy, Clone, Debug)]
pub struct SnowflakeIdGen {
    /// epoch used by the snowflake algorithm.
    epoch: SystemTime,

    /// last_time_millis, last time generate id is used times millis.
    last_time_millis: i64,

    /// instance, is use to supplement id machine or sectionalization attribute.
    pub instance: i32,

    /// auto-increment record.
    idx: u16,
}

impl SnowflakeIdGen {
    pub fn new(instance: i32) -> SnowflakeIdGen {
        Self::with_epoch(instance, UNIX_EPOCH)
    }

    pub fn with_epoch(instance: i32, epoch: SystemTime) -> SnowflakeIdGen {
        //TODO:limit the maximum of input args machine_id and node_id
        let last_time_millis = get_time_millis(epoch);

        SnowflakeIdGen {
            epoch,
            last_time_millis,
            instance,
            idx: 0,
        }
    }

    pub fn generate(&mut self) -> Option<i64> {
        self.generate_with_millis_fn(get_time_millis)
    }

    fn generate_with_millis_fn<F>(&mut self, f: F) -> Option<i64>
    where
        F: Fn(SystemTime) -> i64,
    {
        let now_millis = f(self.epoch);

        if now_millis == self.last_time_millis {
            if self.idx >= 4095 {
                return None;
            }
        } else {
            self.last_time_millis = now_millis;
            self.idx = 0;
        }

        self.idx += 1;

        Some(self.last_time_millis << 22 | ((self.instance << 12) as i64) | (self.idx as i64))
    }
}

#[inline(always)]
/// Get the latest milliseconds of the clock.
pub fn get_time_millis(epoch: SystemTime) -> i64 {
    SystemTime::now()
        .duration_since(epoch)
        .expect("The epoch is later then now")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    const TOTAL_IDS: usize = IDS_PER_THREAD * THREAD_COUNT;
    const THREAD_COUNT: usize = 16;
    const IDS_PER_THREAD: usize = 2_000;

    #[test]
    fn no_duplication_between_multiple_threads() {
        let generator = Arc::new(Mutex::new(SnowflakeIdGen::with_epoch(0, SystemTime::now())));

        let mut result = iter::repeat(generator)
            .enumerate()
            .take(THREAD_COUNT)
            .map(|data| thread::spawn(move || generate_many_ids(data)))
            // This collect makes it so the we don't go through all the threads one by one!!!
            .collect::<Vec<_>>()
            .into_iter()
            .fold(Vec::with_capacity(TOTAL_IDS), |mut vec, thread| {
                vec.append(&mut thread.join().unwrap());
                vec
            });

        result.sort();
        result.dedup();

        assert_eq!(TOTAL_IDS, result.len());
    }

    fn generate_many_ids((thread, generator): (usize, Arc<Mutex<SnowflakeIdGen>>)) -> Vec<i64> {
        (0..IDS_PER_THREAD)
            .map(|cycle| loop {
                let mut lock = generator.lock().unwrap();

                if let Some(id) = lock.generate() {
                    break id;
                }
                println!("Thread {thread} Cycle {cycle}: idx for current time has been filled!");
                drop(lock);
                thread::sleep(Duration::from_millis(1));
            })
            // .inspect(|x| println!("{x:b}"))
            .collect::<Vec<_>>()
    }

    #[test]
    fn fail_after_4095() {
        let mut generator = SnowflakeIdGen::with_epoch(0, SystemTime::now());

        for _ in 1..=4095 {
            let id = generator.generate_with_millis_fn(|_| 0);
            assert!(matches!(id, Some(_)));
        }

        assert_eq!(generator.generate_with_millis_fn(|_| 0), None);
    }
}
