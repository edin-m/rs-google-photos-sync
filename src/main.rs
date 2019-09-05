use std::fs::File;
use std::{thread, time};
use std::option::Option;
use std::sync::mpsc;
use std::collections::HashMap;
use std::string::ToString;
use std::vec::Vec;
use std::borrow::BorrowMut;

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

mod my_db;
mod google_api;
mod error;
mod util;
mod google_photos;
mod downloader;

// =============

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct StoredItem {
    pub mediaItem: MediaItem,
    pub appData: Option<AppData>
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaItem {
    pub id: String,
    pub baseUrl: String,
    pub filename: String,
    pub mediaMetadata: MediaMetaData
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct MediaMetaData {
    pub creationTime: DateTime<Utc>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppData {
    pub download_info: Option<DownloadInfo>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadInfo {
    pub downloaded_at: DateTime<Utc>
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
        select_files_for_download();
    }
}

fn search_and_store_items(num_days_back: i32, limit_hint: usize) {
    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.get_token();

    let mut storage = StoredItemStore::new("secrets/photos.data");

    let media_items: Vec<MediaItem> = google_photos::search(
        &token, num_days_back, limit_hint
    );

    println!("media items {}", media_items.len());

    for media_item in media_items {
        let id = media_item.id.to_owned();

        if let Some(mut stored_item) = storage.get_cloned(&id) {
            stored_item.mediaItem = media_item;
            storage.set(&id, stored_item);
        } else {
            storage.set(&id, StoredItem {
                mediaItem: media_item,
                appData: None
            });
        }
    }

    storage.persist();
}

fn select_files_for_download() {
    let mut storage = StoredItemStore::new("secrets/photos.data");

    let items = storage.filter_values(|(k, v)| {
        v.appData.is_none()
    });

    println!("{}", items.len());
}
