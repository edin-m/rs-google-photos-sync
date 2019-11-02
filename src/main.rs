extern crate chrono;
extern crate commander;
#[macro_use]
extern crate nickel;
extern crate opener;
extern crate reqwest;
extern crate scoped_threadpool;
#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate cron;

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::option::Option;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::vec::Vec;

use chrono::{DateTime, Utc};
use commander::Commander;
use job_scheduler::{Job, JobScheduler, Schedule};

use crate::error::CustomResult;
use crate::google_api::GoogleAuthApi;
use crate::google_photos::GooglePhotosApi;

mod downloader;
mod error;
mod google_api;
mod google_photos;
mod my_db;
mod util;

// ==== CONFIG ===

// refresh token schedule
const REFRESH_SCHEDULE: &'static str = "0/30 * * * * *";
// search job schedule
const SEARCH_SCHEDULE: &'static str = "0 0/20 * * * *";
// download job schedule
const DOWNLOAD_SCHEDULE: &'static str = "0 0/5 * * * *";

// search job -- search new images from x days back
const SEARCH_DAYS_BACK: i32 = 10;
// search job -- search item number limit
const SEARCH_LIMIT: usize = 100000;
// download job -- download file number at the same time
const DOWNLOAD_FILES: i32 = 10;

// =============

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct StoredItem {
    pub mediaItem: MediaItem,
    pub appData: Option<AppData>,
    pub alt_filename: Option<String>,
}

impl StoredItem {
    pub fn get_filename(&self) -> String {
        if let Some(alt_filename) = &self.alt_filename {
            alt_filename.to_owned()
        } else {
            self.mediaItem.filename.to_owned()
        }
    }
}

pub type MediaItemId = String;

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaItem {
    pub id: MediaItemId,
    pub baseUrl: String,
    pub filename: String,
    pub mediaMetadata: MediaMetaData
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaMetaData {
    pub creationTime: DateTime<Utc>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub photo: Option<Photo>,
    pub video: Option<Video>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Photo {}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Video {}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppData {
    pub download_info: Option<DownloadInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadInfo {
    pub downloaded_at: DateTime<Utc>,
}

pub type StoredItemStore = my_db::KeyValueStore<StoredItem>;

fn main() -> CustomResult<()> {
    let command = Commander::new()
        .usage_desc("Read-only sync Google Photos onto a local disk")
        .option_list("-s, --search", "[days back] [limit] Search and store media items", None)
        .option_list("-d, --download", "[num files] Download media items", None)
        .parse_env_or_exit();

    let (tx, rx) = mpsc::channel();

    if let Some(search_params) = command.get_list("search") {
        let days_back = search_params.get(0).unwrap().parse::<i32>()?;
        let limit_hint = search_params.get(1).unwrap().parse::<usize>()?;
        println!("search params {} {}", days_back, limit_hint);

        tx.send(JobTask::SearchFilesTask(days_back, limit_hint)).unwrap();
        drop(tx);

    } else if let Some(download_params) = command.get_list("download") {
        let num_items = download_params.get(0).unwrap().parse::<i32>()?;
        println!("download params {}", num_items);

        tx.send(JobTask::DownloadFilesTask(num_items)).unwrap();
        drop(tx);
    } else {
        run_job_scheduler(tx.clone())?;
    }

    let storage = StoredItemStore::new("secrets/photos.data");
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.authenticate_or_renew()?;

    let photos_api = GooglePhotosApi { token };

    let res = run_task_receiver(&rx, App {
        google_auth,
        photos_api,
        storage,
    });

    match res {
        Err(err) => panic!("Error {}", err.to_string()),
        _ => {}
    }

    Ok(())
}

struct App {
    pub google_auth: GoogleAuthApi,
    pub photos_api: GooglePhotosApi,
    pub storage: StoredItemStore
}

impl App {
    pub fn search(&mut self, num_days_back: i32, limit_hint: usize) -> CustomResult<()> {
        let media_items = self.photos_api.search(num_days_back, limit_hint)?;
        println!("media items {}", media_items.len());
        self.on_media_items(media_items)?;

        Ok(())
    }

    pub fn download(&mut self, num_files: i32) -> CustomResult<()> {
        const NUMBER_OF_FILES_PER_BATCH: i32 = 50;

        let groups = num_files / NUMBER_OF_FILES_PER_BATCH;
        let remainder = num_files % NUMBER_OF_FILES_PER_BATCH;

        for i in 0..groups {
            println!("Split num of files {}x{}+{}, group {}",
                     groups, NUMBER_OF_FILES_PER_BATCH, remainder, i
            );
            self._fine_grade_download(NUMBER_OF_FILES_PER_BATCH)?;
        }

        if remainder > 0 {
            self._fine_grade_download(remainder)?;
        }

        Ok(())
    }

    fn _fine_grade_download(&mut self, num_files: i32) -> CustomResult<()> {
        let selected_stored_items = self.storage.select_files_for_download(num_files);
        let selected_ids = extract_media_item_ids(&selected_stored_items);

        println!("selected {}", selected_ids.len());

        let updated_media_items =
            self.photos_api.batch_get(&selected_ids)?;

        let updated_ids = extract_media_item_ids(&updated_media_items);

        let stored_items = self.get_stored_items_by_ids(&updated_ids);

        let downloaded_ids = downloader::download(&stored_items)?;

        let hash: HashSet<&MediaItemId> = HashSet::from_iter(downloaded_ids.iter());

        let mark_downloaded = updated_media_items
            .iter()
            .filter(|item| {
                hash.contains(&item.id)
            })
            .map(|item| { item.id.to_owned() })
            .collect::<Vec<_>>();

        self.storage.mark_downloaded(&mark_downloaded);

        self.on_media_items(updated_media_items)?;
        Ok(())
    }

    fn get_stored_items_by_ids(&self, ids: &Vec<MediaItemId>) -> Vec<StoredItem> {
        let mut stored_items = Vec::new();

        for id in ids {
            if let Some(a) = self.storage.get_cloned(&id) {
                stored_items.push(a);
            }
        }

        stored_items
    }

    fn on_media_items(&mut self, media_items: Vec<MediaItem>) -> CustomResult<()> {
        for media_item in media_items {
            let id = media_item.id.to_owned();

            if let Some(mut stored_item) = self.storage.get_cloned(&id) {
                stored_item.mediaItem = media_item;
                self.storage.set(&id, stored_item);
            } else {
                self.storage.set(
                    &id,
                    StoredItem {
                        mediaItem: media_item,
                        appData: None,
                        alt_filename: None,
                    },
                );
            }
        }

        self.fix_duplicate_filenames();

        self.storage.persist()?;

        Ok(())
    }

    fn fix_duplicate_filenames(&mut self) {
        type Filename = String;
        let mut map: HashMap<Filename, Vec<MediaItemId>> = HashMap::new();

        for stored_item in self.storage.get_all() {
            let filename = stored_item.mediaItem.filename.to_owned();

            if let Some(ids) = map.get_mut(&filename) {
                ids.push(filename);
            } else {
                map.insert(filename, vec![stored_item.get_media_item_id()]);
            }
        }

        let dups = map
            .iter()
            .filter(|&(_, v)| {
                v.len() > 1
            }).collect::<HashMap<_, _>>();

        let dup_size = dups.len();
        println!("Found {} duplicate files", dup_size);

        for (_, id) in dups {
            let mut i = 0;
            while i < id.len() {
                let id = id.get(i).unwrap();
                if let Some(mut item) = self.storage.get_cloned(&id) {
                    if item.alt_filename.is_none() {
                        item.alt_filename = Some(format!("{}_{}", i, item.mediaItem.filename));
                        self.storage.set(&item.get_media_item_id(), item);
                    }
                }

                i += 1;
            }
        }
    }

    pub fn refresh_token(&mut self) -> CustomResult<()> {
        self.google_auth.authenticate_or_renew()?;

        Ok(())
    }
}

trait AppStorage {
    fn select_files_for_download(&self, limit: i32) -> Vec<StoredItem>;

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>);
}

impl AppStorage for StoredItemStore {
    fn select_files_for_download(&self, limit: i32) -> Vec<StoredItem> {
        self.filter_values(|(_, v)| {
            let mut result = true;

            if let Some(app_data) = &v.appData {
                if let Some(_) = &app_data.download_info {
                    result = false;
                } else {
                    result = true;
                }
            }

            result
        }, Some(limit as usize))
    }

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>) {
        for id in media_item_ids {
            if let Some(mut stored_item) = self.get_cloned(&id) {
                let app_data = AppData {
                    download_info: Some(DownloadInfo {
                        downloaded_at: Utc::now()
                    })
                };
                stored_item.appData = Some(app_data);
                self.set(&id, stored_item);
            }
        }
    }
}

trait HasMediaItemId {
    fn get_media_item_id(&self) -> MediaItemId;
}

fn extract_media_item_ids<T>(items: &Vec<T>) -> Vec<MediaItemId>
    where T: HasMediaItemId
{
    items.iter().map(|item| item.get_media_item_id()).collect::<Vec<_>>()
}

impl HasMediaItemId for MediaItem {
    fn get_media_item_id(&self) -> MediaItemId {
        self.id.to_owned()
    }
}

impl HasMediaItemId for StoredItem {
    fn get_media_item_id(&self) -> MediaItemId {
        self.mediaItem.id.to_owned()
    }
}

enum JobTask {
    RefreshTokenTask,
    DownloadFilesTask(i32),
    SearchFilesTask(i32, usize)
}

fn run_job_scheduler(tx: Sender<JobTask>) -> CustomResult<()> {
    let refresh_task_schedule: Schedule = String::from(REFRESH_SCHEDULE).parse()?;
    let search_task_schedule: Schedule = SEARCH_SCHEDULE.parse()?;
    let download_task_schedule: Schedule = DOWNLOAD_SCHEDULE.parse()?;

    thread::spawn(move || {
        let mut sched = JobScheduler::new();

        sched.add(Job::new(refresh_task_schedule, || {
            tx.send(JobTask::RefreshTokenTask).unwrap();
        }));

        sched.add(Job::new(search_task_schedule, || {
            tx.send(JobTask::SearchFilesTask(SEARCH_DAYS_BACK, SEARCH_LIMIT)).unwrap();
        }));

        sched.add(Job::new(download_task_schedule, || {
            tx.send(JobTask::DownloadFilesTask(DOWNLOAD_FILES)).unwrap();
        }));

        loop {
            sched.tick();

            std::thread::sleep(Duration::from_millis(500));
        }
    });

    Ok(())
}

fn run_task_receiver(rx: &Receiver<JobTask>, mut app: App) -> CustomResult<()> {
    for r in rx {
        match r {
            JobTask::RefreshTokenTask => {
                println!("RefreshTokenTask");
                app.refresh_token()?;
            },
            JobTask::DownloadFilesTask(num_files) => {
                println!("DownloadFilesTask");
                app.download(num_files)?;
            },
            JobTask::SearchFilesTask(num_days_back, limit_hint) => {
                println!("SearchFilesTask");
                app.search(num_days_back, limit_hint)?;
            }
        }
    }

    Ok(())
}
