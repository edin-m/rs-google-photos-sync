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

use std::option::Option;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::vec::Vec;
use std::iter::FromIterator;

use chrono::{DateTime, Utc};
use commander::Commander;
use job_scheduler::{Job, JobScheduler};

use crate::error::{CustomResult};
use crate::google_api::GoogleAuthApi;
use crate::google_photos::GooglePhotosApi;
use std::collections::{HashSet, HashMap};

mod downloader;
mod error;
mod google_api;
mod google_photos;
mod my_db;
mod util;

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
    pub width: String,
    pub height: String,
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
        .option_list("-s, --search", "Search and store media items", None)
        .option_list("-d, --download", "Download media items", None)
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
        run_job_scheduler(tx.clone());
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
        let selected_stored_items = self.storage.select_files_for_download(num_files);
        let selected_ids = extract_media_item_ids(&selected_stored_items);

        let updated_media_items =
            self.photos_api.batch_get(&selected_ids)?;

        let updated_ids = extract_media_item_ids(&updated_media_items);

        let mut stored_items = Vec::new();

        for id in updated_ids {
            if let Some(a) = self.storage.get_cloned(&id) {
                stored_items.push(a);
            }
        }

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

        for v in self.storage.get_all() {
            let filename = v.get_filename();

            if let Some(ids) = map.get_mut(&filename) {
                ids.push(filename);
            } else {
                map.insert(filename, vec![v.get_media_item_id()]);
            }
        }

        let dups = map
            .iter()
            .filter(|&(_, v)| {
                v.len() > 1
            }).collect::<HashMap<_, _>>();

        let dup_size = dups.len();
        println!("Found {} duplicate files", dup_size * 2);

        for (_, v) in map {
            let mut i = 0;
            while i < v.len() {
                let id = v.get(i).unwrap();
                if let Some(mut item) = self.storage.get_cloned(&id) {
                    item.alt_filename = Some(format!("{}_{}", i, item.get_filename()));
                    self.storage.set(&item.get_media_item_id(), item);
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

fn run_job_scheduler(tx: Sender<JobTask>) {
    thread::spawn(move || {
        let mut sched = JobScheduler::new();

        let search_days_back = 10;
        let search_limit = 10000;

        let download_files = 10;

        sched.add(Job::new("1/30 * * * * *".parse().unwrap(), || {
            tx.send(JobTask::RefreshTokenTask).unwrap();
        }));

        sched.add(Job::new("* 5 * * * *".parse().unwrap(), || {
            tx.send(JobTask::SearchFilesTask(search_days_back, search_limit)).unwrap();
        }));

        sched.add(Job::new("* 1 * * * *".parse().unwrap(), || {
            tx.send(JobTask::DownloadFilesTask(download_files)).unwrap();
        }));

        loop {
            sched.tick();

            std::thread::sleep(Duration::from_millis(500));
        }
    });
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
