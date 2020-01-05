use std::sync::mpsc::{Sender};
use std::thread;
use std::time::Duration;

use job_scheduler::{Job, JobScheduler, Schedule};

use crate::error::CustomResult;

pub enum JobTask {
    RefreshTokenTask,
    DownloadFilesTask(i32),
    SearchFilesTask(i32, usize),
}

pub fn run_job_scheduler(tx: Sender<JobTask>) -> CustomResult<()> {
    let config = crate::config::Config::new()?;

    let refresh_task_schedule: Schedule = String::from(config.refresh_token_schedule.to_owned()).parse()?;
    let search_task_schedule: Schedule = config.search_new_items_schedule.parse()?;
    let download_task_schedule: Schedule = config.download_photos_schedule.parse()?;

    thread::spawn(move || {
        let mut sched = JobScheduler::new();

        sched.add(Job::new(refresh_task_schedule, || {
            tx.send(JobTask::RefreshTokenTask).unwrap();
        }));

        sched.add(Job::new(search_task_schedule, || {
            tx.send(JobTask::SearchFilesTask(config.search_days_back, config.search_limit)).unwrap();
        }));

        sched.add(Job::new(download_task_schedule, || {
            tx.send(JobTask::DownloadFilesTask(config.download_files_parallel)).unwrap();
        }));

        loop {
            sched.tick();

            std::thread::sleep(Duration::from_millis(500));
        }
    });

    Ok(())
}
