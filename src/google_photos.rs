use std::convert::{From, Into};
use std::option::Option;

use chrono::{Datelike, DateTime, Duration, TimeZone, Utc};
use reqwest::{ClientBuilder, header::HeaderMap, header::HeaderValue};
use reqwest::header;
use serde::{Deserialize, Serialize};

use crate::{MediaItem};
use crate::downloader::DownloadUrl;
use crate::error::{CustomError, CustomResult};
use crate::google_api::GoogleToken;
use crate::util;

pub struct GooglePhotosApi {
    pub token: GoogleToken
}

impl GooglePhotosApi {
    pub fn search(&self, num_days_back: i32, limit_hint: usize) -> CustomResult<Vec<MediaItem>> {
        search(&self.token, num_days_back, limit_hint)
    }

    pub fn batch_get(&self, media_item_ids: &Vec<String>) -> CustomResult<Vec<MediaItem>> {
        batch_get(media_item_ids, &self.token)
    }
}

fn search(token: &GoogleToken, num_days_back: i32, limit_hint: usize)
              -> CustomResult<Vec<MediaItem>>
{
    let mut headers = HeaderMap::new();
    let token = format!("Bearer {}", token.token.access_token);
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&token)?,
    );

    let client = ClientBuilder::new().default_headers(headers).build()?;

    let mut media_items = Vec::<MediaItem>::new();
    let mut page_token: Option<String> = None;

    while media_items.len() < limit_hint {
        let range = DateRange::range_from_days(num_days_back);
        let search_filter = SearchFilter {
            dateFilter: range.into(),
            includeArchivedMedia: true,
        };

        let search_request = SearchRequest {
            pageSize: 100,
            pageToken: page_token,
            filters: search_filter,
        };

        let mut resp: SearchResponse = client
            .post("https://photoslibrary.googleapis.com/v1/mediaItems:search")
            .json(&search_request).send()?.json()?;

        println!("search result {} items", resp.mediaItems.len());

        if let Some(next_page_token) = resp.nextPageToken {
            page_token = Some(next_page_token.to_owned());
        } else {
            page_token = None
        }

        media_items.append(&mut resp.mediaItems);
    }

    Ok(media_items)
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

impl From<DateRange> for DateFilter {
    fn from(range: DateRange) -> Self {
        DateFilter { ranges: vec![range] }
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

fn batch_get(media_item_ids: &Vec<String>, google_token: &GoogleToken) -> CustomResult<Vec<MediaItem>> {
    const MAX_GOOGLE_BATCH_GET_SIZE: usize = 50;

    let groups = util::split_into_groups(media_item_ids, MAX_GOOGLE_BATCH_GET_SIZE);
    println!("split {} items into {} groups", media_item_ids.len(), groups.len());

    let mut got = Vec::new();

    for group in groups {
        let items = _batch_get(&group, google_token)?;

        println!("fetched {}", items.len());

        for item in items {
            got.push(item);
        }
    }

    Ok(got)
}

fn _batch_get(media_item_ids: &Vec<&String>, google_token: &GoogleToken) -> CustomResult<Vec<MediaItem>> {
    let mut url = String::from("https://photoslibrary.googleapis.com/v1/mediaItems:batchGet?");

    for media_item_id in media_item_ids {
        url = url + &format!("mediaItemIds={}&", media_item_id);
    }

    let mut headers = HeaderMap::new();
    let token = format!("Bearer {}", google_token.token.access_token);
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&token)?,
    );

    let client = ClientBuilder::new().default_headers(headers).build()?;

    let res: BatchGetResult = client.get(url.as_str()).send()?.json()?;

    Ok(res.mediaItemResults
        .into_iter()
        .map(|v| v.mediaItem)
        .collect::<Vec<_>>())
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

impl DownloadUrl for MediaItem {
    fn create_download_url(&self) -> CustomResult<String> {
        let mut url = String::new();

        if let Some(_) = &self.mediaMetadata.photo {
            let meta = &self.mediaMetadata;
            let w = if meta.width.is_some() { meta.width.as_ref().unwrap() } else { "" };
            let h = if meta.height.is_some() { meta.height.as_ref().unwrap() } else { "" };
            url = url + format!("{}=w{}-h{}", &self.baseUrl, w, h).as_str();
            Ok(url)
        } else {
            println!("video not supported {}", self.id);
            Err(CustomError::Err("video not supported".to_string()))
        }
    }
}
