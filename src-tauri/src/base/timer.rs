use anyhow::{Context, Result};
use chrono::Local;
use delay_timer::prelude::{DelayTimer, DelayTimerBuilder, TaskBuilder};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

type TaskID = u64;
// const AUTO_REFRESH_ID: &str = "auto_refresh_task";

#[derive(Debug, Clone)]
pub struct TimerTask {
    pub task_id: TaskID,
    pub interval_seconds: i64,
    #[allow(unused)]
    // 不知道这个字段有什么用
    pub last_period: i64,
}

pub struct Timer {
    /// cron manager
    pub delay_timer: Arc<RwLock<DelayTimer>>,

    /// save the current state - using RwLock for better read concurrency
    pub timer_map: Arc<RwLock<HashMap<String, TimerTask>>>,

    /// increment id - atomic counter for better performance
    pub timer_count: AtomicU64,

    /// Flag to mark if timer is initialized - atomic for better performance
    pub initialized: AtomicBool,
}


static TIMER_INSTANCE: std::sync::OnceLock<Timer> = std::sync::OnceLock::new();


impl Timer {
    pub fn global() -> &'static Timer {
        TIMER_INSTANCE.get_or_init(|| Self::new())
    }
    fn new() -> Self {
        Timer {
            delay_timer: Arc::new(RwLock::new(DelayTimerBuilder::default().build())),
            timer_map: Arc::new(RwLock::new(HashMap::new())),
            timer_count: AtomicU64::new(1),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize timer with better error handling and atomic operations
    pub fn init(&self) -> Result<()> {
        // Use compare_exchange for thread-safe initialization check
        if self
            .initialized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        // Initialize timer tasks
        if let Err(e) = self.refresh() {
            // Reset initialization flag on error
            self.initialized.store(false, Ordering::SeqCst);
            return Err(e);
        }

        // 定时每一分钟刷新待办动作
        let auto_refrsh_task_id = self.timer_count.fetch_add(1, Ordering::Relaxed);
        let auto_refresh_task = TaskBuilder::default()
            .set_task_id(auto_refrsh_task_id)
            .set_maximum_parallel_runnable_num(1)
            .set_frequency_repeated_by_minutes(1)
            .spawn_async_routine(move || async move {
                let _ = Self::global().refresh();
            })
            .context("failed to create auto_refresh_task")?;
        let delay_timer = self.delay_timer.write();
        delay_timer.add_task(auto_refresh_task)?;

        Ok(())
    }

    /// Refresh timer tasks with better error handling
    pub fn refresh(&self) -> Result<()> {
        // Generate diff outside of lock to minimize lock contention
        // Hub::global().refresh();
        let diff_map = self.gen_diff();
        if diff_map.is_empty() {
            return Ok(());
        }

        // Apply changes while holding locks
        let mut timer_map = self.timer_map.write();
        let mut delay_timer = self.delay_timer.write();

        for (uid, diff) in diff_map {
            match diff {
                DiffFlag::Del(tid) => {
                    timer_map.remove(&uid);
                    let _ = delay_timer.remove_task(tid);
                }
                DiffFlag::Add(tid, interval) => {
                    let now = Local::now().timestamp();
                    let task = TimerTask {
                        task_id: tid,
                        interval_seconds: interval,
                        last_period: now,
                    };

                    timer_map.insert(uid.clone(), task);
                    if self.add_task(&mut delay_timer, uid.clone(), tid, interval, now + interval).is_err() {
                        timer_map.remove(&uid);
                    }
                }
                DiffFlag::Mod(tid, interval) => {
                    let _ = delay_timer.remove_task(tid);
                    let now = Local::now().timestamp();
                    let task = TimerTask {
                        task_id: tid,
                        interval_seconds: interval,
                        last_period: now,
                    };

                    timer_map.insert(uid.clone(), task);
                    if self.add_task(&mut delay_timer, uid.clone(), tid, interval, now + interval).is_err() {
                        timer_map.remove(&uid);
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate map of profile UIDs to update intervals
    fn gen_map(&self) -> HashMap<String, i64> {
        let new_map = HashMap::new();
        // TODO: 从外部数据源获取定时任务配置，填充 new_map
        new_map
    }

    // Generate differences between current and new timer configuration
    fn gen_diff(&self) -> HashMap<String, DiffFlag> {
        let mut diff_map = HashMap::new();
        let new_map = self.gen_map();

        // Read lock for comparing current state
        let timer_map = self.timer_map.read();


        // Find tasks to modify or delete
        for (uid, timer_task) in timer_map.iter() {
            match new_map.get(uid) {
                // 由于delay_timer内部会更新task的interval_seconds，所以这里应该会不断发送ModFlag
                Some(&interval) if interval != timer_task.interval_seconds => {
                    // Task exists but interval changed

                    diff_map.insert(uid.clone(), DiffFlag::Mod(timer_task.task_id, interval));
                }
                None => {
                    // Task no longer needed

                    diff_map.insert(uid.clone(), DiffFlag::Del(timer_task.task_id));
                }
                _ => {
                    // Task exists with same interval, no change needed

                }
            }
        }

        // Find new tasks to add
        // 我去，你这task_id竟然是自增的吗
        let mut next_id = self.timer_count.load(Ordering::Relaxed);
        let original_id = next_id;

        for (uid, &interval) in new_map.iter() {
            if !timer_map.contains_key(uid) {
                diff_map.insert(uid.clone(), DiffFlag::Add(next_id, interval));
                next_id += 1;
            }
        }

        // Update counter only if we added new tasks
        if next_id > original_id {
            self.timer_count.store(next_id, Ordering::Relaxed);
        }

        diff_map
    }

    /// Add a timer task with better error handling
    fn add_task(
        &self,
        delay_timer: &mut DelayTimer,
        uid: String,
        tid: TaskID,
        seconds: i64,
        timestamp: i64,
    ) -> Result<()> {


        // Create a task with reasonable retries and backoff
        let task = TaskBuilder::default()
            .set_task_id(tid)
            .set_maximum_parallel_runnable_num(1)
            .set_frequency_once_by_seconds(seconds as u64)
            .spawn_async_routine(move || {
                let uid = uid.clone();
                async move {
                    Self::async_task(uid, timestamp).await;
                }
            })
            .context("failed to create timer task")?;

        delay_timer
            .add_task(task)
            .context("failed to add timer task")?;
        Ok(())
    }

    // Async task with better error handling and logging
    async fn async_task(id: String, timestamp: i64) {
        // TODO: 实现定时任务的执行逻辑
        let _task_start = std::time::Instant::now();
        let _ = (id, timestamp);
    }
}

#[derive(Debug)]
enum DiffFlag {
    Del(TaskID),
    Add(TaskID, i64),
    Mod(TaskID, i64),
}