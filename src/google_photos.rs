use std::clone::Clone;
use std::cmp::min;
use std::collections::HashMap;
use std::convert::{From, Into};
use std::fs::File;
use std::io::{BufReader, Write, copy};
use std::marker::Copy;
use std::ops::Add;
use std::option::Option;
use std::result;
use std::sync::mpsc;
use std::thread;
use std::time;
use std::time::Instant;

use chrono::{DateTime, Datelike, Duration, FixedOffset, TimeZone, Utc};

use reqwest::header;
use reqwest::{header::HeaderMap, header::HeaderValue, Client, ClientBuilder, Response};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;

use crate::google_api::GoogleToken;
use crate::{MediaItem, Photo};

pub fn search(token: &GoogleToken, num_days_back: i32, limit_hint: usize) -> Vec<MediaItem> {
    let mut headers = HeaderMap::new();
    let token = format!("Bearer {}", token.token.access_token);
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&token).unwrap(),
    );

    let client = ClientBuilder::new()
        .default_headers(headers)
        .build()
        .unwrap();

    let mut media_items = Vec::<MediaItem>::new();
    let mut page_token: Option<String> = None;

    while media_items.len() < limit_hint {
        // TODO: don't generate filter each time
        let range = DateRange::range_from_days(num_days_back);
        let search_filter = SearchFilter {
            dateFilter: DateFilter {
                ranges: vec![range],
            },
            includeArchivedMedia: true,
        };

        let mut search_request = SearchRequest {
            pageSize: 100,
            pageToken: page_token,
            filters: search_filter,
        };

        let mut resp: SearchResponse = client
            .post("https://photoslibrary.googleapis.com/v1/mediaItems:search")
            .json(&search_request)
            .send()
            .unwrap()
            .json()
            .unwrap();

        println!("search result {} items", resp.mediaItems.len());

        if let Some(nextPageToken) = resp.nextPageToken {
            page_token = Some(nextPageToken.to_owned());
        } else {
            page_token = None
        }

        media_items.append(&mut resp.mediaItems);
    }

    media_items
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct SearchResponse {
    mediaItems: Vec<MediaItem>,
    nextPageToken: Option<String>,
}

#[derive(Serialize, Debug)]
#[allow(non_snake_case)]
struct SearchRequest {
    pageSize: i32,
    pageToken: Option<String>,
    filters: SearchFilter,
}

#[derive(Serialize, Debug)]
#[allow(non_snake_case)]
struct SearchFilter {
    dateFilter: DateFilter,
    includeArchivedMedia: bool,
}

#[derive(Serialize, Debug)]
struct DateFilter {
    ranges: Vec<DateRange>,
}

#[derive(Serialize, Debug)]
#[allow(non_snake_case)]
struct DateRange {
    startDate: Date,
    endDate: Date,
}

impl DateRange {
    pub fn range_from_days(num_days_back: i32) -> DateRange {
        let end = Utc::now();
        let start = end - Duration::days(num_days_back as i64);

        DateRange {
            startDate: start.into(),
            endDate: end.into(),
        }
    }
}

#[derive(Serialize, Debug)]
struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl<T: TimeZone> From<DateTime<T>> for Date {
    fn from(date_time: DateTime<T>) -> Self {
        Date {
            year: date_time.year(),
            month: date_time.month(),
            day: date_time.day(),
        }
    }
}

pub fn batch_get(media_item_ids: &Vec<String>, google_token: &GoogleToken) -> Vec<MediaItem> {
    let groups = split_into_groups(media_item_ids, 50);

    println!(
        "split {} items into {} groups",
        media_item_ids.len(),
        groups.len()
    );

    let mut got = Vec::new();

    for group in groups {
        let items = _batch_get(&group, google_token);

        println!("fetched {}", items.len());

        for item in items {
            got.push(item);
        }
    }

    got
}

fn _batch_get(media_item_ids: &Vec<&String>, google_token: &GoogleToken) -> Vec<MediaItem> {
    let mut url = String::from("https://photoslibrary.googleapis.com/v1/mediaItems:batchGet?");

    for media_item_id in media_item_ids {
        url = url + &format!("mediaItemIds={}&", media_item_id);
    }

    let mut headers = HeaderMap::new();
    let token = format!("Bearer {}", google_token.token.access_token);
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&token).unwrap(),
    );

    let client = ClientBuilder::new()
        .default_headers(headers)
        .build()
        .unwrap();

    let res: BatchGetResult = client.get(url.as_str()).send().unwrap().json().unwrap();

    res.mediaItemResults
        .into_iter()
        .map(|v| v.mediaItem)
        .collect::<Vec<_>>()
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct BatchGetResult {
    pub mediaItemResults: Vec<MediaItemResult>,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct MediaItemResult {
    pub mediaItem: MediaItem,
}

pub fn download_files(media_items: &Vec<MediaItem>, google_token: &GoogleToken) {
    for media_item in media_items {
        let url = create_download_url(media_item, google_token);

        let mut headers = HeaderMap::new();
        let token = format!("Bearer {}", google_token.token.access_token);

        let client = ClientBuilder::new()
            .default_headers(headers)
            .build().unwrap();

        println!("downloading url {} to {}", url, media_item.filename);

        let mut resp = client.get(url.as_str()).send().unwrap();
        let mut dest = File::create(&media_item.filename).unwrap();
        copy(&mut resp, &mut dest).unwrap();
    }
}

fn create_download_url(media_item: &MediaItem, google_token: &GoogleToken) -> String {
    let mut url = String::new();

    if let Some(photo) = &media_item.mediaMetadata.photo {
        url = url + format!(
            "{}=w{}-h{}",
            media_item.baseUrl,
            media_item.mediaMetadata.width,
            media_item.mediaMetadata.height
        ).as_str();
    } else {
        panic!("video not supported");
    }

    url
}

fn split_into_groups(items: &Vec<String>, group_size: usize) -> Vec<Vec<&String>> {
    let mut groups = Vec::new();

    let num_groups = (items.len() as f32 / group_size as f32).ceil() as usize;

    for i in 0..num_groups {
        let mut vec = Vec::new();

        let start = i * group_size;
        let end = min(items.len(), i * group_size + group_size);

        for j in start..end {
            vec.push(items.get(j).unwrap());
        }

        groups.push(vec);
    }

    groups
}
