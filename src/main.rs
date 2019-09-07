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
use std::vec::Vec;

use chrono::{DateTime, Utc};
use commander::Commander;

use crate::error::CustomResult;
//use crate::google_api::GoogleAuthApi;
use crate::google_photos::GooglePhotosApi;


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

    if let Some(job) = command.get_str("job") {
        println!("job name {}", job);
    }

    let storage = StoredItemStore::new("secrets/photos.data");
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.authenticate_or_renew()?;

    let photos_api = GooglePhotosApi { token };
    let mut app = App { photos_api, storage };

    if let Some(search_params) = command.get_list("search") {
        let days_back = search_params.get(0).unwrap().parse::<i32>()?;
        let limit_hint = search_params.get(1).unwrap().parse::<usize>()?;
        println!("search params {} {}", days_back, limit_hint);
        app.search(days_back, limit_hint)?;

    } else if let Some(download_params) = command.get_list("download") {
        let num_items = download_params.get(0).unwrap().parse::<i32>()?;
        println!("download params {}", num_items);
        app.download(num_items)?;

    } else {
//        run_jobs();
    }

    Ok(())
}

struct App {
//    google_auth: GoogleAuthApi,
    photos_api: GooglePhotosApi,
    storage: StoredItemStore
}

impl App {
    pub fn search(&mut self, num_days_back: i32, limit_hint: usize) -> CustomResult<()> {
        let media_items = self.photos_api.search(num_days_back, limit_hint)?;
        println!("media items {}", media_items.len());
        self.on_media_items(media_items)?;
        Ok(())
    }

    pub fn download(&mut self, _: i32) -> CustomResult<()> {
        let selected_stored_items = self.storage.select_files_for_download();
        println!("selected {} files to download", selected_stored_items.len());

        let media_item_ids = extract_media_item_ids(&selected_stored_items);

        let media_items = self.photos_api.batch_get(&media_item_ids)?;
        let _ = self.photos_api.download_files(&media_items)?;
        self.storage.mark_downloaded(&media_items);
        self.on_media_items(media_items)?;
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
        println!("fix_duplicate_filenames not implemented!")
    }
}

trait AppStorage {
    fn select_files_for_download(&self) -> Vec<StoredItem>;

    fn mark_downloaded(&mut self, media_items: &Vec<MediaItem>);
}

impl AppStorage for StoredItemStore {
    fn select_files_for_download(&self) -> Vec<StoredItem> {
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
        })
    }

    fn mark_downloaded(&mut self, media_items: &Vec<MediaItem>) {
        let ids = media_items
            .iter()
            .map(|item| item.id.to_owned())
            .collect::<Vec<_>>();

        for id in ids {
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

fn extract_media_item_ids(stored_items: &Vec<StoredItem>) -> Vec<MediaItemId> {
    stored_items.iter().map(|stored_item| stored_item.mediaItem.id.to_owned()).collect::<Vec<_>>()
}
