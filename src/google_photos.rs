use std::result;
use std::time;
use std::time::Instant;
use std::fs::File;
use std::io::{BufReader, Write};
use std::option::Option;
use std::clone::Clone;
use std::marker::Copy;
use std::ops::Add;
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;
use std::convert::{Into, From};

use chrono::{DateTime, Utc, FixedOffset, TimeZone, Duration, Datelike};

use reqwest::header;
use reqwest::{header::HeaderMap, header::HeaderValue, Client, ClientBuilder, Response};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;

use crate::google_api::GoogleToken;
use crate::MediaItem;

pub fn search(token: &GoogleToken, num_days_back: i32, limit_hint: usize) -> Vec<MediaItem> {
    let mut headers = HeaderMap::new();
    let token = format!("Bearer {}", token.token.access_token);
    headers.insert(header::AUTHORIZATION, HeaderValue::from_str(&token).unwrap());

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
            dateFilter: DateFilter::from_range(range),
            includeArchivedMedia: true,
        };

        let mut search_request = SearchRequest {
            pageSize: 100,
            pageToken: page_token,
            filters: search_filter,
        };

        let mut resp: SearchResponse = client.post("https://photoslibrary.googleapis.com/v1/mediaItems:search")
            .json(&search_request)
            .send().unwrap().json().unwrap();

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
    ranges: Vec<DateRange>
}

impl DateFilter {
    pub fn from_range(range: DateRange) -> DateFilter {
        let mut filter = DateFilter {
            ranges: Vec::new()
        };

        filter.ranges.push(range);

        filter
    }
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

