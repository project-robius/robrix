use std::{collections::{btree_map::Entry, BTreeMap}, ops::{Deref, DerefMut}, sync::{Arc, Mutex}, time::SystemTime};
use makepad_widgets::{error, log, Cx, SignalToUI};
use matrix_sdk::{media::{MediaFormat, MediaRequestParameters}, ruma::{events::room::MediaSource, OwnedMxcUri}};
use crate::{home::room_screen::TimelineUpdate, image_viewer::ImageViewerAction, sliding_sync::{self, MatrixRequest, OnMediaFetchedFn}, utils::MEDIA_THUMBNAIL_FORMAT};


/// An entry in the media cache.
#[derive(Debug, Clone, Default)]
pub enum MediaCacheEntry {
    Null,
    #[default] NotInitialized,
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
    cache: BTreeMap<OwnedMxcUri, (Option<OwnedMxcUri>, MediaCacheEntryRef, MediaCacheEntryRef)>,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}

impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, (Option<OwnedMxcUri>, MediaCacheEntryRef, MediaCacheEntryRef)>;
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

    pub fn set_keys(&mut self, original_uri: &OwnedMxcUri, thumbnail_uri: Option<OwnedMxcUri>) {
        if let Some(uri) = thumbnail_uri.clone() {
            if let Entry::Vacant(v) = self.cache.entry(uri.clone()) {
                v.insert((None, Arc::new(Mutex::new(MediaCacheEntry::Null)), Arc::new(Mutex::new(MediaCacheEntry::default()))));
            }

            if let Entry::Vacant(v) = self.cache.entry(original_uri.clone()) {
                v.insert((thumbnail_uri.clone(), Arc::new(Mutex::new(MediaCacheEntry::Null)), Arc::new(Mutex::new(MediaCacheEntry::default()))));
            }

        } else {
            if let Entry::Vacant(v) = self.cache.entry(original_uri.clone()) {
                v.insert((None, Arc::new(Mutex::new(MediaCacheEntry::default())), Arc::new(Mutex::new(MediaCacheEntry::default()))));
            }
        }
    }
    /// Tries to get the media from the cache, or submits an async request to fetch it.
    ///
    /// This method *does not* block or wait for the media to be fetched,
    /// and will return `MediaCache::Requested` while the async request is in flight.
    /// If a request is already in flight, this will not issue a new redundant request.
    ///
    /// * If the `media_format` is requesting a thumbnail that is not yet in the cache,
    ///   this function will fetch the thumbnail, and return the full-size image (if it exists).
    /// * If the `media_format` is requesting a full-size image that is not yet in the cache,
    ///   this function will fetch the full-size image, and return a thumbnail (if it exists).
    ///
    /// Returns a tuple of the media cache entry and the media format of that cached entry.
    pub fn try_get_media_or_fetch(
            &mut self,
            mxc_uri: &OwnedMxcUri,
            prefer_thumbnail: bool,
            on_fetched: OnMediaFetchedFn,
        ) -> MediaCacheEntry {
            let mut ret = MediaCacheEntry::Requested;
            let (thumbnail_uri, better_entry, entry) = self.cache.get(mxc_uri).unwrap().clone();
            let better_entry = better_entry.lock().unwrap().clone();
            let entry = entry.lock().unwrap().clone();
            if let MediaCacheEntry::Loaded(_) | MediaCacheEntry::Failed = entry {
                // Case 1
                if matches!(better_entry, MediaCacheEntry::Null) {
                    return entry;
                }
                // Case 1
            }

            let (should_fetch, destination, format) = if let Some(thumbnail_uri) = thumbnail_uri.as_ref() {
                // Case2
                {
                    let (_, _, thumbnail_ref) = self.cache.get_mut(thumbnail_uri).unwrap();
                    let mut mg = thumbnail_ref.lock().unwrap();
                    match mg.clone() {
                        MediaCacheEntry::NotInitialized => { *mg = MediaCacheEntry::Requested }
                        MediaCacheEntry::Loaded(_) | MediaCacheEntry::Failed => { ret = mg.clone() }
                        _ => { }
                    }
                }
                let (_, _, entry_ref) = self.cache.get_mut(mxc_uri).unwrap();

                (true, entry_ref.clone(), MediaFormat::File)
                // Case2
            }
            else {
                // Case 3 & 4
                if prefer_thumbnail {
                    // Case 3
                    let (_, _, entry_ref) = self.cache.get_mut(mxc_uri).unwrap();
                    let mut mg = entry_ref.lock().unwrap();
                    match mg.clone() {
                        MediaCacheEntry::NotInitialized => { *mg = MediaCacheEntry::Requested }
                        MediaCacheEntry::Loaded(_) | MediaCacheEntry::Failed => { return mg.clone(); }
                        _ => { }
                    }
                    (true, entry_ref.clone(), MEDIA_THUMBNAIL_FORMAT.into())
                    // Case 3
                } else {
                    // Case4
                    let (_, better_entry_ref, entry_ref) = self.cache.get_mut(mxc_uri).unwrap();
                    let mut better_entry_ref_mg = better_entry_ref.lock().unwrap();
                    let entry_ref_mg = entry_ref.lock().unwrap();

                    match better_entry_ref_mg.clone() {
                        MediaCacheEntry::Loaded(_) => {
                            return better_entry_ref_mg.clone()
                        }
                        MediaCacheEntry::NotInitialized => {
                            if let MediaCacheEntry::Loaded(_) | MediaCacheEntry::Failed = entry_ref_mg.clone() {
                                ret = entry_ref_mg.clone()
                            }
                            *better_entry_ref_mg = MediaCacheEntry::Requested;
                            (true, better_entry_ref.clone(), MediaFormat::File)
                        }
                        _ => { panic!("") }
                    }
                    // Case4
                }
            // Case 3 & 4
            };

            if should_fetch {
                sliding_sync::submit_async_request(
                    MatrixRequest::FetchMedia {
                        media_request: MediaRequestParameters {
                            source: MediaSource::Plain(mxc_uri.clone()),
                            format
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
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();
            Cx::post_action(ImageViewerAction::Show(data.clone()));
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
