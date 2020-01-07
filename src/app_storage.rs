use chrono::{Utc};

use crate::{StoredItem, MediaItemId, StoredItemStore, AppData, DownloadInfo, HasMediaItemId};
use std::collections::HashMap;

pub type MediaItemIdByFileName = HashMap<String, MediaItemId>;

pub trait AppStorage {
    fn get_media_item_id_by_file_name(&self) -> MediaItemIdByFileName;

    fn select_files_for_download(&self, limit: usize) -> Vec<StoredItem>;

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>);
}

impl AppStorage for StoredItemStore {
    fn get_media_item_id_by_file_name(&self) -> MediaItemIdByFileName {
        let mut map = MediaItemIdByFileName::with_capacity(self.data.len());

        for (_, val) in self.data.iter() {
            map.insert(val.get_filename(), val.get_media_item_id());
        }

        map
    }

    fn select_files_for_download(&self, limit: usize) -> Vec<StoredItem> {
        self.filter_values(|(_, v)| {
            let mut result = true;

            if let Some(app_data) = &v.appData {
                result = app_data.has_download_info();
            }

            result
        }, Some(limit))
    }

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>) {
        for id in media_item_ids {
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
