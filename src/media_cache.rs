use std::{collections::{btree_map::Entry, BTreeMap}, ops::{Deref, DerefMut}, sync::{Arc, Mutex}, time::SystemTime};
use makepad_widgets::{error, log, Cx, SignalToUI};
use matrix_sdk::{media::{MediaFormat, MediaRequestParameters}, ruma::{events::room::MediaSource, OwnedMxcUri}};
use crate::{home::room_screen::TimelineUpdate, image_viewer::ImageViewerAction, sliding_sync::{self, MatrixRequest, OnMediaFetchedFn}, utils::MEDIA_THUMBNAIL_FORMAT};


/// An entry in the media cache.
#[derive(Debug, Clone)]
pub enum MediaCacheEntry {
    NotInitialized,
    /// A request has been issued and we're waiting for it to complete.
    Requested,
    /// The media has been successfully loaded from the server.
    Loaded(Arc<[u8]>),
    /// The media failed to load from the server.
    Failed,
}

/// A reference to a media cache entry and its associated format.
pub type MediaCacheEntryRef = Arc<Mutex<MediaCacheEntry>>;

/// A cache of fetched media, indexed by Matrix URI.
///
/// A single Matrix URI may have multiple media formats associated with it,
/// such as a thumbnail and a full-size image.
pub struct MediaCache {
    /// The actual cached data.
    cache: BTreeMap<OwnedMxcUri, (Option<OwnedMxcUri>, MediaCacheEntryRef)>,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}

impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, (Option<OwnedMxcUri>, MediaCacheEntryRef)>;
    fn deref(&self) -> &Self::Target {
        &self.cache
    }
}

impl DerefMut for MediaCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cache
    }
}

impl MediaCache {
    /// Creates a new media cache that will use the given media format
    /// when fetching media from the server.
    ///
    /// It will also optionally send updates to the given timeline update sender
    /// when a media request has completed.
    pub const fn new(
        timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            timeline_update_sender,
        }
    }

    /// This function only for images or stickers.
    /// Never call this function on populating audio, video, etc.
    pub fn image_set_keys(&mut self, original_uri: &OwnedMxcUri, thumbnail_uri: Option<&OwnedMxcUri>) {
        if let Entry::Vacant(v) = self.cache.entry(original_uri.clone()) {
            v.insert((thumbnail_uri.cloned(), Arc::new(Mutex::new(MediaCacheEntry::NotInitialized))));
        }

        if let Some(thumbnail_uri) = thumbnail_uri {
            if let Entry::Vacant(v) = self.cache.entry(thumbnail_uri.clone()) {
                v.insert((None, Arc::new(Mutex::new(MediaCacheEntry::Requested))));
            }
        }
    }

    /// Returns the media cache entry.
    pub fn try_get_media_or_fetch(
        &mut self,
        mxc_uri: &OwnedMxcUri,
        on_fetched: OnMediaFetchedFn,
    ) -> MediaCacheEntry {
        let mut ret = MediaCacheEntry::Requested;

        let (should_fetch, destination, format_to_fetch ) = match self.cache.entry(mxc_uri.clone()) {
            Entry::Vacant(v) => {
                (true, v.insert((None, Arc::new(Mutex::new(MediaCacheEntry::Requested)))).1.clone(), MediaFormat::File)
            }
            Entry::Occupied(mut o) => {
                let (thumbnail_uri, entry_ref) = o.get().clone();
                let er = entry_ref.lock().unwrap().clone();

                match er.clone() {
                    MediaCacheEntry::Loaded(_) | MediaCacheEntry::Failed => {
                        return er.clone();
                    }
                    MediaCacheEntry::NotInitialized | MediaCacheEntry::Requested => {
                        if let MediaCacheEntry::NotInitialized = er.clone() {
                            *o.get_mut().1.lock().unwrap() = MediaCacheEntry::Requested
                        }
                        match thumbnail_uri.as_ref() {
                            Some(uri) => {
                                ret = self.cache.get(uri).unwrap().1.lock().unwrap().clone();
                                (true, entry_ref.clone(), MediaFormat::File)
                            }
                            None => {
                                (true, entry_ref.clone(), MEDIA_THUMBNAIL_FORMAT.into())
                            }
                        }
                    }
                }
            }
        };

        if should_fetch {
            sliding_sync::submit_async_request(
                MatrixRequest::FetchMedia {
                    media_request: MediaRequestParameters {
                        source: MediaSource::Plain(mxc_uri.clone()),
                        format: format_to_fetch
                    },
                    on_fetched,
                    destination,
                    update_sender: self.timeline_update_sender.clone(),
                }
            );
        }
        ret
    }
}

/// Insert data into a previously-requested media cache entry.
pub fn insert_into_cache<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<MediaCacheEntry>,
    _request: MediaRequestParameters,
    data: matrix_sdk::Result<D>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();

            // debugging: dump out the media image to disk
            if false {
                if let MediaSource::Plain(mxc_uri) = _request.source {
                    log!("Fetched media for {mxc_uri}");
                    let mut path = crate::temp_storage::get_temp_dir_path().clone();
                    let filename = format!("{}_{}_{}",
                        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                        mxc_uri.server_name().unwrap(), mxc_uri.media_id().unwrap(),
                    );
                    path.push(filename);
                    path.set_extension("png");
                    log!("Writing user media image to disk: {:?}", path);
                    std::fs::write(path, &data)
                        .expect("Failed to write user media image to disk");
                }
            }
            MediaCacheEntry::Loaded(data)
        }
        Err(e) => {
            error!("Failed to fetch media for {:?}: {e:?}", _request.source);
            MediaCacheEntry::Failed
        }
    };

    *value_ref.lock().unwrap() = new_value;

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched);
    }
    SignalToUI::set_ui_signal();
}


pub fn image_viewer_insert_into_cache<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<MediaCacheEntry>,
    _request: MediaRequestParameters,
    data: matrix_sdk::Result<D>,
    _update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();
            // This function just simply copy from `insert_from_cache`,
            // only here is different, we just post an action on getting the image data.
            Cx::post_action(ImageViewerAction::ReplaceImage(data.clone()));
            log!("thumbnail data len: {}", data.len());
            MediaCacheEntry::Loaded(data)
        }
        Err(e) => {
            error!("Failed to fetch media for {:?}: {e:?}", _request.source);
            MediaCacheEntry::Failed
        }
    };

    *value_ref.lock().unwrap() = new_value;

    SignalToUI::set_ui_signal();
}
