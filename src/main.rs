extern crate chrono;
extern crate commander;
extern crate cron;
extern crate ctrlc;
extern crate flexi_logger;
extern crate log;
#[macro_use]
extern crate nickel;
extern crate opener;
extern crate reqwest;
extern crate scoped_threadpool;
#[macro_use]
extern crate serde;
extern crate serde_json;
#[cfg(windows)]
extern crate winapi;

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::option::Option;
use std::path::Path;
use std::process::Command;
use std::sync::{mpsc, Arc};
use std::sync::mpsc::Receiver;
use std::vec::Vec;

use chrono::{DateTime, Utc};
use commander::Commander;
use log::{error, info, trace, warn};

use app_storage::AppStorage;
use scheduling::JobTask;

use crate::config::Config;
use crate::error::{CustomError, CustomResult};
use crate::google_api::GoogleAuthApi;
use crate::google_photos::GooglePhotosApi;
use std::sync::atomic::{AtomicBool, Ordering};

mod downloader;
mod error;
mod google_api;
mod google_photos;
mod my_db;
mod util;
mod config;
mod app_storage;
mod scheduling;
mod service;

// =============
// TODO: test periodic save db to file
// TODO: improve fix renamed files
// TODO: add log4rs

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct StoredItem {
    pub mediaItem: MediaItem,
    pub appData: Option<AppData>,
    pub alt_filename: Option<String>,
}

impl StoredItem {
    fn get_filename(&self) -> String {
        if let Some(alt_filename) = &self.alt_filename {
            alt_filename.to_owned()
        } else {
            self.mediaItem.filename.to_owned()
        }
    }

    fn is_marked_downloaded(&self) -> bool {
        if let Some(app_data) = &self.appData {
            app_data.download_info.is_some()
        } else {
            false
        }
    }

    fn mark_downloaded(&mut self) {
        self.appData = Some(AppData {
            download_info: Some(DownloadInfo {
                downloaded_at: Utc::now()
            })
        });
    }

    fn unmark_downloaded(&mut self) {
        self.appData = None;
    }
}

pub type MediaItemId = String;

pub type FileName = String;

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaItem {
    pub id: MediaItemId,
    pub baseUrl: String,
    pub filename: String,
    pub mediaMetadata: MediaMetaData,
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
    flexi_logger::Logger::with_str("info")
        .format(flexi_logger::detailed_format)
        .start()
        .unwrap();

    let command = Commander::new()
        .usage_desc("Read-only sync Google Photos onto a local disk")
        .option_list("-s, --search", "[days back] [limit] Search and store media items", None)
        .option_list("-d, --download", "[num files] Download media items", None)
        .option("--winservice", "When running as a windows service", None)
        // TODO: remove
        .option("--install", "install windows service", None)
        .option("--uninstall-service", "install windows service", None)
        .parse_env_or_exit();

    println!("is win service");
    if let Some(value) = command.get("winservice") {
        if value {
            println!("win service");
        }
    }

    if let Some(install) = command.get("install") {
        if install {
            println!("requested install service");
            service::install_service();
        }
    } else if let Some(uninstall) = command.get("uninstall") {
        if uninstall {
            println!("requested uninstall service");
            service::uninstall_service();
        }
    } else  {
        return main_ex(&command);
    }

    Ok(())
}

fn main_ex(command: &Commander) -> CustomResult<()> {
    let storage = StoredItemStore::new("secrets/photos.data");
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.authenticate_or_renew()?;

    let photos_api = GooglePhotosApi { token };

    let mut app = App {
        google_auth,
        photos_api,
        storage,
    };

    mark_unmark_downloaded_photos_in_fs(&mut app)
        .map_err(|e| CustomError::Err(format!("could not mark/unmark downloaded {}", e)))?;

    let (tx, rx) = mpsc::channel();

    if let Some(search_params) = command.get_list("search") {
        let days_back = search_params.get(0).unwrap().parse::<i32>()?;
        let default_limit = String::from("999999");
        let limit_hint = search_params.get(1).unwrap_or(&default_limit).parse::<usize>()?;
        println!("search params days_back:{} limit:{}", days_back, limit_hint);

        tx.send(JobTask::SearchFilesTask(days_back, limit_hint)).unwrap();
        drop(tx);
    } else if let Some(download_params) = command.get_list("download") {
        let num_items = download_params.get(0).unwrap().parse::<i32>()?;
        println!("download params {}", num_items);

        tx.send(JobTask::DownloadFilesTask(num_items)).unwrap();
        drop(tx);
    } else {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_cloned = stop_flag.clone();
        // TODO: no need for ctrlc
//        ctrlc::set_handler(move || {
//            println!("RECEIVED CTRLC");
//            stop_flag.store(true, Ordering::SeqCst);
//        }).expect("Error setting ctrl-c handle");
        scheduling::run_job_scheduler(tx, stop_flag_cloned)?;
    }

    let res = run_task_receiver(&rx, app);

    match res {
        Err(err) => panic!("Error {}", err.to_string()),
        _ => {}
    }

    Ok(())
}

pub struct MarkDownloadedPartition {
    mark_downloaded: Vec<MediaItemId>,
    unmark_downloaded: Vec<MediaItemId>
}

fn mark_unmark_downloaded_photos_in_fs(app: &mut App) -> CustomResult<()>
{
    let config = Config::new()?;
    let downloaded = get_downloaded_files()?;
    println!("Total # of files in fs: {}", downloaded.len());

    let partition = app.storage.partition_by_marked_download(&downloaded);

    if config.fix_downloaded_info.mark_downloaded {
        println!("{} to be mark downloaded", partition.mark_downloaded.len());
        app.storage.mark_downloaded(&partition.mark_downloaded);
    }

    if config.fix_downloaded_info.unmark_downloaded {
        println!("{} to be unmark downloaded", partition.unmark_downloaded.len());
        app.storage.unmark_downloaded(&partition.unmark_downloaded);
    }

    app.storage.persist()?;

    Ok(())
}

fn get_downloaded_files() -> CustomResult<Box<HashSet<FileName>>>
{
    let config = Config::new()?;
    let path = Path::new(config.storage_location.as_str());

    let mut file_names = Box::new(HashSet::new());

    if path.exists() {
        for entry in path.read_dir()? {
            if let Ok(entry) = entry {
                if entry.file_type()?.is_file() {
                    if let Some(file_name) = entry.file_name().into_string().ok() {
                        file_names.insert(file_name);
                    }
                }
            }
        }
    }

    Ok(file_names)
}

struct App {
    pub google_auth: GoogleAuthApi,
    pub photos_api: GooglePhotosApi,
    pub storage: StoredItemStore,
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
            self.download_files(NUMBER_OF_FILES_PER_BATCH)?;
        }

        if remainder > 0 {
            self.download_files(remainder)?;
        }

        Ok(())
    }

    fn download_files(&mut self, num_files: i32) -> CustomResult<()> {
        let selected_stored_items = self.storage.select_files_for_download(num_files as usize);
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
                hash.contains(&item.id.to_owned())
            })
            .map(|item| { item.get_media_item_id() })
            .collect::<Vec<_>>();

        self.storage.mark_downloaded(&mark_downloaded);
        self.on_media_items(updated_media_items)?;

        Ok(())
    }

    fn get_stored_items_by_ids(&self, ids: &Vec<MediaItemId>) -> Vec<StoredItem> {
        let mut stored_items = Vec::with_capacity(ids.len());

        for id in ids {
            if let Some(a) = self.storage.get_cloned(&id) {
                stored_items.push(a);
            }
        }

        stored_items
    }

    fn on_media_items(&mut self, media_items: Vec<MediaItem>) -> CustomResult<()> {
        for media_item in media_items {
            let id = media_item.get_media_item_id();

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
        println!("Found {} duplicate file names", dup_size);

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

fn run_task_receiver(rx: &Receiver<JobTask>, mut app: App) -> CustomResult<()> {
    for r in rx {
        match r {
            JobTask::RefreshTokenTask => {
                println!("RefreshTokenTask");
                app.refresh_token()?;
            }
            JobTask::DownloadFilesTask(num_files) => {
                println!("DownloadFilesTask");
                app.download(num_files)?;
            }
            JobTask::SearchFilesTask(num_days_back, limit_hint) => {
                println!("SearchFilesTask");
                app.search(num_days_back, limit_hint)?;
            }
        }
    }

    Ok(())
}
