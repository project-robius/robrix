use std::{cell::RefCell, ops::{Deref, DerefMut}, sync::{Arc, Mutex}, time::SystemTime};
use hashbrown::{hash_map::RawEntryMut, HashMap};
use makepad_widgets::{error, log, Cx, SignalToUI};
use matrix_sdk::{media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings}, ruma::{events::room::MediaSource, OwnedMxcUri}, Error, HttpError};
use reqwest::StatusCode;
use crate::{home::room_screen::TimelineUpdate, sliding_sync::{self, MatrixRequest}};

/// The value type in the media cache, one per Matrix URI.
#[derive(Debug, Clone)]
pub struct MediaCacheValue {
    full_file: Option<MediaCacheEntryRef>,
    thumbnail: Option<(MediaCacheEntryRef, MediaThumbnailSettings)>,
}

/// An entry in the media cache.
#[derive(Debug, Clone)]
pub enum MediaCacheEntry {
    /// A request has been issued and we're waiting for it to complete.
    /// The vector contains the list of timeline update senders that should be notified.
    Requested(Vec<crossbeam_channel::Sender<TimelineUpdate>>),
    /// The media has been successfully loaded from the server.
    Loaded(Arc<[u8]>),
    /// The media failed to load from the server with reqwest status code.
    Failed(StatusCode),
}

/// A reference to a media cache entry and its associated format.
pub type MediaCacheEntryRef = Arc<Mutex<MediaCacheEntry>>;

thread_local! {
    /// A global cache of fetched media, indexed by Matrix URI.
    ///
    /// This cache is shared across all rooms to avoid re-fetching the same media.
    static GLOBAL_MEDIA_CACHE: RefCell<MediaCache> = RefCell::new(MediaCache::new());
}

/// A cache of fetched media, indexed by Matrix URI.
///
/// A single Matrix URI may have multiple media formats associated with it,
/// such as a thumbnail and a full-size image.
pub struct MediaCache {
    /// The actual cached data.
    /// We use `hashbrown::HashMap` to enable the `raw_entry` API, which allows
    /// looking up entries by reference (`&OwnedMxcUri`) without cloning the key.
    cache: HashMap<OwnedMxcUri, MediaCacheValue>,
}
impl Deref for MediaCache {
    type Target = HashMap<OwnedMxcUri, MediaCacheValue>;
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
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
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
        requested_format: MediaFormat,
        update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> (MediaCacheEntry, MediaFormat) {
        // We initialize the return value with the `Requested` state, which is the default.
        // We clone the sender so that we can pass it to the background task if needed.
        let mut post_request_retval = (
            MediaCacheEntry::Requested(update_sender.clone().map_or_else(Vec::new, |s| vec![s])),
            requested_format.clone(),
        );
        let entry_ref_to_fetch: MediaCacheEntryRef;

        let entry = self.cache.raw_entry_mut().from_key(mxc_uri);

        match entry {
            RawEntryMut::Occupied(mut occupied) => {
                let value = occupied.get_mut();
                match requested_format {
                    MediaFormat::Thumbnail(ref requested_mts) => {
                        if let Some((entry_ref, existing_mts)) = value.thumbnail.as_ref() {
                            let entry_val = entry_ref.lock().unwrap().deref().clone();
                            // If the entry is currently in the `Requested` state, we add the sender to the list.
                            if let MediaCacheEntry::Requested(ref mut senders) = *entry_ref.lock().unwrap() {
                                if let Some(sender) = update_sender {
                                    senders.push(sender);
                                }
                            }
                            return (
                                entry_val,
                                MediaFormat::Thumbnail(existing_mts.clone()),
                            );
                        } else {
                            // Here, a thumbnail was requested but not found, so fetch it.
                            let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested(
                                update_sender.map_or_else(Vec::new, |s| vec![s])
                            )));
                            value.thumbnail = Some((Arc::clone(&entry_ref), requested_mts.clone()));
                            // If a full-size image is already loaded, return it.
                            if let Some(existing_file) = value.full_file.as_ref() {
                                if let MediaCacheEntry::Loaded(d) = existing_file.lock().unwrap().deref() {
                                    post_request_retval = (
                                        MediaCacheEntry::Loaded(Arc::clone(d)),
                                        MediaFormat::File,
                                    );
                                }
                            }
                            entry_ref_to_fetch = entry_ref;
                        }
                    }
                    MediaFormat::File => {
                        if let Some(entry_ref) = value.full_file.as_ref() {
                            let entry_val = entry_ref.lock().unwrap().deref().clone();
                            // If the entry is currently in the `Requested` state, we add the sender to the list.
                            if let MediaCacheEntry::Requested(ref mut senders) = *entry_ref.lock().unwrap() {
                                if let Some(sender) = update_sender {
                                    senders.push(sender);
                                }
                            }
                            return (
                                entry_val,
                                MediaFormat::File,
                            );
                        } else {
                            // Here, a full-size image was requested but not found, so fetch it.
                            let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested(
                                update_sender.map_or_else(Vec::new, |s| vec![s])
                            )));
                            value.full_file = Some(entry_ref.clone());
                            // If a thumbnail is already loaded, return it.
                            if let Some((existing_thumbnail, existing_mts)) = value.thumbnail.as_ref() {
                                if let MediaCacheEntry::Loaded(d) = existing_thumbnail.lock().unwrap().deref() {
                                    post_request_retval = (
                                        MediaCacheEntry::Loaded(Arc::clone(d)),
                                        MediaFormat::Thumbnail(existing_mts.clone()),
                                    );
                                }
                            }
                            entry_ref_to_fetch = entry_ref;
                        }
                    }
                }
            }
            RawEntryMut::Vacant(vacant) => {
                let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested(
                    update_sender.map_or_else(Vec::new, |s| vec![s])
                )));
                let value = match &requested_format {
                    MediaFormat::Thumbnail(requested_mts) => MediaCacheValue {
                        full_file: None,
                        thumbnail: Some((Arc::clone(&entry_ref), requested_mts.clone())),
                    },
                    MediaFormat::File => MediaCacheValue {
                        full_file: Some(Arc::clone(&entry_ref)),
                        thumbnail: None,
                    },
                };
                vacant.insert(mxc_uri.clone(), value);
                entry_ref_to_fetch = entry_ref;
            }
        }

        sliding_sync::submit_async_request(MatrixRequest::FetchMedia {
            media_request: MediaRequestParameters {
                source: MediaSource::Plain(mxc_uri.clone()),
                format: requested_format,
            },
            on_fetched: insert_into_cache,
            destination: entry_ref_to_fetch,
            update_sender: None,
        });
        post_request_retval
    }

    /// Removes a specific media format from the cache for the given MXC URI.
    /// If `format` is None, removes the entire cache entry for the URI.
    /// Returns the removed cache entry if found, None otherwise.
    pub fn remove_cache_entry(&mut self, mxc_uri: &OwnedMxcUri, format: Option<MediaFormat>) -> Option<MediaCacheEntryRef> {
        match format {
            Some(MediaFormat::Thumbnail(_)) => {
                if let Some(cache_value) = self.cache.get_mut(mxc_uri) {
                    if let Some((removed_entry, _)) = cache_value.thumbnail.take() {
                        // If both thumbnail and full_file are None, remove the entire entry
                        if cache_value.full_file.is_none() {
                            self.cache.remove(mxc_uri);
                        }
                        return Some(removed_entry);
                    }
                }
                None
            }
            Some(MediaFormat::File) => {
                if let Some(cache_value) = self.cache.get_mut(mxc_uri) {
                    if let Some(removed_entry) = cache_value.full_file.take() {
                        // If both thumbnail and full_file are None, remove the entire entry
                        if cache_value.thumbnail.is_none() {
                            self.cache.remove(mxc_uri);
                        }
                        return Some(removed_entry);
                    }
                }
                None
            }
            None => {
                // Remove the entire entry for this MXC URI
                self.cache.remove(mxc_uri).map(|cache_value| {
                    // Return the full_file entry if it exists, otherwise the thumbnail entry
                    cache_value.full_file
                        .or_else(|| cache_value.thumbnail.map(|(entry, _)| entry))
                        .unwrap_or_else(|| Arc::new(Mutex::new(MediaCacheEntry::Requested(Vec::new()))))
                })
            }
        }
    }
}

/// Helper function to access the global media cache.
pub fn get_or_fetch_media(
    _cx: &mut Cx,
    mxc_uri: &OwnedMxcUri,
    requested_format: MediaFormat,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) -> (MediaCacheEntry, MediaFormat) {
    GLOBAL_MEDIA_CACHE.with_borrow_mut(|cache| {
        cache.try_get_media_or_fetch(mxc_uri, requested_format, update_sender)
    })
}

/// Helper function to remove media from the global cache.
pub fn remove_media(
    _cx: &mut Cx,
    mxc_uri: &OwnedMxcUri,
    format: Option<MediaFormat>,
) -> Option<MediaCacheEntryRef> {
    GLOBAL_MEDIA_CACHE.with_borrow_mut(|cache| {
        cache.remove_cache_entry(mxc_uri, format)
    })
}

/// Converts a Matrix SDK error to a MediaCacheEntry::Failed with appropriate status codes.
fn error_to_media_cache_entry(error: Error, request: &MediaRequestParameters) -> MediaCacheEntry {
    match error {
        Error::Http(http_error) => {
            if let Some(client_error) = http_error.as_client_api_error() {
                error!("Client error for media cache: {client_error} for request: {:?}", request);
                MediaCacheEntry::Failed(client_error.status_code)
            } else {
                match *http_error {
                    HttpError::Reqwest(reqwest_error) => {
                        // Checking if the connection is timeout is not important as Matrix SDK has implemented maximum timeout duration.
                        if !reqwest_error.is_connect() {
                            MediaCacheEntry::Failed(StatusCode::INTERNAL_SERVER_ERROR)
                        } else if reqwest_error.is_status() {
                            MediaCacheEntry::Failed(reqwest_error
                                .status()
                                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                        } else {
                            MediaCacheEntry::Failed(StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                    _ => MediaCacheEntry::Failed(StatusCode::NOT_FOUND),
                }
            }
        }
        Error::InsufficientData => MediaCacheEntry::Failed(StatusCode::PARTIAL_CONTENT),
        Error::AuthenticationRequired => MediaCacheEntry::Failed(StatusCode::UNAUTHORIZED),
        _ => MediaCacheEntry::Failed(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<MediaCacheEntry>,
    request: MediaRequestParameters,
    data: matrix_sdk::Result<D>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();

            // debugging: dump out the media image to disk
            if false {
                if let MediaSource::Plain(mxc_uri) = &request.source {
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
        Err(e) => error_to_media_cache_entry(e, &request)
    };

    let mut guard = value_ref.lock().unwrap();
    let senders = if let MediaCacheEntry::Requested(s) = &*guard {
        s.clone()
    } else {
        Vec::new()
    };
    *guard = new_value;

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched(request.clone()));
    }
    for sender in senders {
        let _ = sender.send(TimelineUpdate::MediaFetched(request.clone()));
    }
    SignalToUI::set_ui_signal();
}
