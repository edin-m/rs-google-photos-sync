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
use std::vec::Vec;

use chrono::{DateTime, Utc};
use commander::Commander;

use crate::error::CustomResult;
use crate::google_api::GoogleAuthApi;
use crate::google_photos::GooglePhotosApi;

mod downloader;
mod error;
mod google_api;
mod google_photos;
mod my_db;
mod util;
mod config;
mod app_storage;
mod scheduling;

use app_storage::AppStorage;
use scheduling::JobTask;
use std::sync::mpsc::Receiver;
use crate::config::Config;
use std::path::Path;

// =============
// TODO: test periodic save db to file

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
}

pub type MediaItemId = String;

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

impl AppData {
    fn has_download_info(&self) -> bool {
        self.download_info.is_none()
    }
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
        scheduling::run_job_scheduler(tx.clone())?;
    }

    let storage = StoredItemStore::new("secrets/photos.data");
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.authenticate_or_renew()?;

    let photos_api = GooglePhotosApi { token };

    let mut app = App {
        google_auth,
        photos_api,
        storage,
    };

    scan_fs_and_populate_db(&mut app)?;

    let res = run_task_receiver(&rx, app);

    match res {
        Err(err) => panic!("Error {}", err.to_string()),
        _ => {}
    }

    Ok(())
}

// TODO: what if downloaded file is not complete
// -- download to .tmp and rename after completion
fn scan_fs_and_populate_db(app: &mut App) -> CustomResult<()>
{
    let config = Config::new()?;
    let path = Path::new(config.storage_location.as_str());

    let map = app.storage.get_media_item_id_by_file_name();

    let mut mark_downloaded: Vec<MediaItemId> = Vec::new();

    for entry in path.read_dir()? {
        if let Ok(entry) = entry {
            if entry.file_type()?.is_file() {
                let file_name = entry.file_name().into_string().unwrap_or(String::from("unknown"));

                if let Some(id) = map.get(&file_name) {
                    if let Some(stored_item) = app.storage.get(id) {
                        if !stored_item.is_marked_downloaded() {
                            mark_downloaded.push(id.to_owned());
                        }
                    }
                }
            }
        }
    }

    println!("Found {} items not marked as downloaded. Marking.", mark_downloaded.len());
    app.storage.mark_downloaded(&mark_downloaded);

    Ok(())
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
        const NUMBER_OF_FILES_PER_BATCH: i32 = 20;

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

        let downloaded_ids = downloader::download(&selected_stored_items)?;

        let hash: HashSet<&MediaItemId> = HashSet::from_iter(downloaded_ids.iter());
        let mark_downloaded = selected_stored_items
            .iter()
            .filter(|item| {
                hash.contains(&item.mediaItem.id)
            })
            .map(|item| { item.get_media_item_id() })
            .collect::<Vec<_>>();

        self.storage.mark_downloaded(&mark_downloaded);
        self.storage.persist()?;

        Ok(())
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

trait HasMediaItemId {
    fn get_media_item_id(&self) -> MediaItemId;
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
