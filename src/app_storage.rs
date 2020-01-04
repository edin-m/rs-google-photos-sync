use chrono::{Utc};

use crate::{StoredItem, MediaItemId, StoredItemStore, AppData, DownloadInfo};

pub trait AppStorage {
    fn select_files_for_download(&self, limit: i32) -> Vec<StoredItem>;

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>);
}

impl AppStorage for StoredItemStore {
    fn select_files_for_download(&self, limit: i32) -> Vec<StoredItem> {
        self.filter_values(|(_, v)| {
            let mut result = true;

            if let Some(app_data) = &v.appData {
                result = app_data.has_download_info();
            }

            result
        }, Some(limit as usize))
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
