use crate::util;
use crate::error::CustomResult;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub refresh_token_schedule: String,
    pub search_new_items_schedule: String,
    pub download_photos_schedule: String,
    pub search_days_back: i32,
    pub search_limit: usize,
    pub download_files_parallel: i32,
    pub storage_location: String
}

impl Config {
    pub fn new() -> CustomResult<Config> {
        let path = "config.json";

        util::read_json_file::<Config>(path.to_owned())
    }
}
