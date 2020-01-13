use std::sync::mpsc::{Sender};
use std::thread;
use std::time::Duration;

use job_scheduler::{Job, JobScheduler, Schedule};

use crate::error::CustomResult;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub enum JobTask {
    RefreshTokenTask,
    DownloadFilesTask(i32),
    SearchFilesTask(i32, usize)
}

pub fn run_job_scheduler(tx: Sender<JobTask>, stop_flag: Arc<AtomicBool>) -> CustomResult<()> {
    let config = crate::config::Config::new()?;

    let refresh_task_schedule: Schedule = String::from(config.refresh_token_schedule.to_owned()).parse()?;
    let search_task_schedule: Schedule = config.search_new_items_schedule.parse()?;
    let download_task_schedule: Schedule = config.download_photos_schedule.parse()?;

    thread::spawn(move || {
        let mut sched = JobScheduler::new();

        let tx1 = tx.clone();
        sched.add(Job::new(refresh_task_schedule, move || {
            tx1.send(JobTask::RefreshTokenTask).unwrap();
        }));

        let tx2 = tx.clone();
        let search_days_back = config.search_days_back;
        let search_limit = config.search_limit;
        sched.add(Job::new(search_task_schedule, move || {
            tx2.send(JobTask::SearchFilesTask(search_days_back, search_limit)).unwrap();
        }));

        let tx3 = tx.clone();
        let download_files_parallel = config.download_files_parallel;
        sched.add(Job::new(download_task_schedule, move || {
            tx3.send(JobTask::DownloadFilesTask(download_files_parallel)).unwrap();
        }));

        loop {
            sched.tick();

            std::thread::sleep(Duration::from_millis(500));

            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    Ok(())
}
