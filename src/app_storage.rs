use std::collections::{HashMap, HashSet};

use crate::{FileName, HasMediaItemId, MarkDownloadedPartition, MediaItemId, StoredItem, StoredItemStore};

pub type MediaItemIdByFileName = HashMap<String, MediaItemId>;

pub trait AppStorage {
    fn get_media_item_id_by_file_name(&self) -> MediaItemIdByFileName;

    fn select_files_for_download(&self, limit: usize) -> Vec<StoredItem>;

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>);

    fn unmark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>);

    fn partition_by_marked_download(&self, fs_file_names: &HashSet<FileName>) -> MarkDownloadedPartition;
}

impl AppStorage for StoredItemStore {
    fn partition_by_marked_download(&self, fs_file_names: &HashSet<FileName>) -> MarkDownloadedPartition {
        let mut partition = MarkDownloadedPartition {
            mark_downloaded: Vec::new(),
            unmark_downloaded: Vec::new()
        };

        self.data.iter().for_each(|(k, v)| {
            let is_in_fs = fs_file_names.contains(v.get_filename().as_str());

            if is_in_fs && !v.is_marked_downloaded() {
                partition.mark_downloaded.push(k.to_owned());
            }

            if !is_in_fs && v.is_marked_downloaded() {
                partition.unmark_downloaded.push(k.to_owned());
            }
        });

        partition
    }

    fn get_media_item_id_by_file_name(&self) -> MediaItemIdByFileName {
        let mut map = MediaItemIdByFileName::with_capacity(self.data.len());

        for (_, val) in self.data.iter() {
            map.insert(val.get_filename(), val.get_media_item_id());
        }

        map
    }

    fn select_files_for_download(&self, limit: usize) -> Vec<StoredItem> {
        self.filter_values(|(_, v)| {
            !v.is_marked_downloaded()
        }, Some(limit))
    }

    fn mark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>) {
        for id in media_item_ids {
            if let Some(stored_item) = self.data.get_mut(id) {
                stored_item.mark_downloaded();
            }
        }
    }

    fn unmark_downloaded(&mut self, media_item_ids: &Vec<MediaItemId>) {
        for id in media_item_ids {
            if let Some(stored_item) = self.data.get_mut(id) {
                stored_item.unmark_downloaded();
            }
        }
    }
}
