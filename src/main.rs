use std::fs::File;
use std::{thread, time};
use std::option::Option;
use std::sync::mpsc;
use std::collections::HashMap;
use std::string::ToString;

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

mod my_db;
mod google_api;
mod error;
mod util;

// =============

const CALLBACK_URL: &'static str = "http://localhost:3001/oauth2redirect";

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
struct MediaItem {
    pub id: String,
    
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
struct StoredItem {
    pub mediaItem: MediaItem
}

fn main() {

    let mut google_auth = google_api::GoogleAuthApi::create();
    let token = google_auth.get_token();

    println!("token {:#?}", token);

    let mut storage = my_db::KeyValueStore::<MediaItem>::new("secrets/photos.data");


}

