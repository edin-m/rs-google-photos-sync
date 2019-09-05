use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fs::File;
use std::io::copy;
use std::option::Option;
use std::string::ToString;
use std::sync::mpsc;
use std::vec::Vec;
use std::{thread, time};
use std::marker::Sync;

extern crate opener;

#[macro_use]
extern crate nickel;

extern crate reqwest;

#[macro_use]
extern crate serde;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate chrono;

extern crate commander;

use commander::Commander;

use chrono::{DateTime, Utc};

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

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaItem {
    pub id: String,
    pub baseUrl: String,
    pub filename: String,
    pub mediaMetadata: MediaMetaData,
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

fn main() {
    let command = Commander::new()
        .usage_desc("Read-only sync Google Photos onto a local disk")
        .option_list("-s, --search", "Search and store media items", None)
        .option_list("-d, --download", "Download media items", None)
        .parse_env_or_exit();

    if let Some(job) = command.get_str("job") {
        println!("job name {}", job);
    }

    if let Some(search_params) = command.get_list("search") {
        let days_back = search_params.get(0).unwrap().parse::<i32>().unwrap();
        let limit_hint = search_params.get(1).unwrap().parse::<usize>().unwrap();
        println!("search params {} {}", days_back, limit_hint);
        search_and_store_items(days_back, limit_hint);
    }

    if let Some(download_params) = command.get_list("download") {
        let num_items = download_params.get(0).unwrap().parse::<i32>().unwrap();
        println!("download params {}", num_items);
        download_files(num_items);
    }
}

fn search_and_store_items(num_days_back: i32, limit_hint: usize) {
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.get_token();

    //    let mut storage = StoredItemStore::new("secrets/photos.data");

    let media_items: Vec<MediaItem> = google_photos::search(&token, num_days_back, limit_hint);

    println!("media items {}", media_items.len());

    let mut storage = StoredItemStore::new("secrets/photos.data");

    on_media_items(media_items, &mut storage);

    //    for media_item in media_items {
    //        let id = media_item.id.to_owned();
    //
    //        if let Some(mut stored_item) = storage.get_cloned(&id) {
    //            stored_item.mediaItem = media_item;
    //            storage.set(&id, stored_item);
    //        } else {
    //            storage.set(&id, StoredItem {
    //                mediaItem: media_item,
    //                appData: None
    //            });
    //        }
    //    }
    //
    //    storage.persist();
}

fn on_media_items(media_items: Vec<MediaItem>, storage: &mut StoredItemStore) {

    for media_item in media_items {
        let id = media_item.id.to_owned();

        if let Some(mut stored_item) = storage.get_cloned(&id) {
            stored_item.mediaItem = media_item;
            storage.set(&id, stored_item);
        } else {
            storage.set(
                &id,
                StoredItem {
                    mediaItem: media_item,
                    appData: None,
                    alt_filename: None,
                },
            );
        }
    }

    //    fix_duplicate_filenames(&storage);

    storage.persist();
}

fn fix_duplicate_filenames(storage: &StoredItemStore) {}

fn download_files(num_items: i32) {
    let mut storage = StoredItemStore::new("secrets/photos.data");

    let stored_items = select_files_for_download(&mut storage);

    let ids = stored_items
        .iter()
        .map(|item| item.mediaItem.id.to_owned())
        .collect::<Vec<_>>();

    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.get_token();

    let v = vec![ids.get(0).unwrap().to_owned()];

    let media_items = google_photos::batch_get(&ids, &token);

    println!("batch get received {}", media_items.len());

    google_photos::download_files(&media_items, &token);

    mark_downloaded(&media_items, &mut storage);

    on_media_items(media_items, &mut storage);
}

fn select_files_for_download(storage: &mut StoredItemStore) -> Vec<StoredItem> {
    let items = storage.filter_values(|(k, v)| {
        let mut result = true;

        if let Some(app_data) = &v.appData {
            if let Some(download_info) = &app_data.download_info {
                result = false;
            } else {
                result = true;
            }
        }

        result
    });

    println!("selected {} files to download", items.len());

    items
}

fn mark_downloaded(media_items: &Vec<MediaItem>, storage: &mut StoredItemStore) {
    let ids = media_items
        .iter()
        .map(|item| item.id.to_owned())
        .collect::<Vec<_>>();

    for id in ids {
        if let Some(mut stored_item) = storage.get_cloned(&id) {
            let app_data = AppData {
                download_info: Some(DownloadInfo {
                    downloaded_at: Utc::now()
                })
            };
            stored_item.appData = Some(app_data);
            storage.set(&id, stored_item);
        }
    }
}
