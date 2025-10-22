use std::{collections::{btree_map::Entry, BTreeMap}, ops::{Deref, DerefMut}, sync::{Arc, Mutex}, time::SystemTime};
use makepad_widgets::{error, log, SignalToUI};
use matrix_sdk::{media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings}, ruma::{events::room::MediaSource, OwnedMxcUri, api::client::error::{Error as ClientError, ErrorBody}}, Error, HttpError};
use reqwest::StatusCode;
use serde_json::json;
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
    Requested,
    /// The media has been successfully loaded from the server.
    Loaded(Arc<[u8]>),
    /// The media failed to load from the server with the ruma client api error.
    Failed(ClientError),
}

/// A reference to a media cache entry and its associated format.
pub type MediaCacheEntryRef = Arc<Mutex<MediaCacheEntry>>;


/// A cache of fetched media, indexed by Matrix URI.
///
/// A single Matrix URI may have multiple media formats associated with it,
/// such as a thumbnail and a full-size image.
pub struct MediaCache {
    /// The actual cached data.
    cache: BTreeMap<OwnedMxcUri, MediaCacheValue>,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}
impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, MediaCacheValue>;
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
        mxc_uri: OwnedMxcUri,
        requested_format: MediaFormat,
    ) -> (MediaCacheEntry, MediaFormat) {
        let mut post_request_retval = (MediaCacheEntry::Requested, requested_format.clone());

        let entry_ref = match self.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => match &requested_format {
                MediaFormat::Thumbnail(requested_mts) => {
                    let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested));
                    vacant.insert(MediaCacheValue {
                        full_file: None,
                        thumbnail: Some((Arc::clone(&entry_ref), requested_mts.clone())),
                    });
                    entry_ref
                },
                MediaFormat::File => {
                    let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested));
                    vacant.insert(MediaCacheValue {
                        full_file: Some(Arc::clone(&entry_ref)),
                        thumbnail: None,
                    });
                    entry_ref
                },
            }
            Entry::Occupied(mut occupied) => match requested_format {
                MediaFormat::Thumbnail(ref requested_mts) => {
                    if let Some((entry_ref, existing_mts)) = occupied.get().thumbnail.as_ref() {
                        return (
                            entry_ref.lock().unwrap().deref().clone(),
                            MediaFormat::Thumbnail(existing_mts.clone()),
                        );
                    }
                    else {
                        // Here, a thumbnail was requested but not found, so fetch it.
                        let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested));
                        occupied.get_mut().thumbnail = Some((Arc::clone(&entry_ref), requested_mts.clone()));
                        // If a full-size image is already loaded, return it.
                        if let Some(existing_file) = occupied.get().full_file.as_ref() {
                            if let MediaCacheEntry::Loaded(d) = existing_file.lock().unwrap().deref() {
                                post_request_retval = (
                                    MediaCacheEntry::Loaded(Arc::clone(d)),
                                    MediaFormat::File,
                                );
                            }
                        }
                        entry_ref
                    }
                }
                MediaFormat::File => {
                    if let Some(entry_ref) = occupied.get().full_file.as_ref() {
                        return (
                            entry_ref.lock().unwrap().deref().clone(),
                            MediaFormat::File,
                        );
                    }
                    else {
                        // Here, a full-size image was requested but not found, so fetch it.
                        let entry_ref = Arc::new(Mutex::new(MediaCacheEntry::Requested));
                        occupied.get_mut().full_file = Some(entry_ref.clone());
                        // If a thumbnail is already loaded, return it.
                        if let Some((existing_thumbnail, existing_mts)) = occupied.get().thumbnail.as_ref() {
                            if let MediaCacheEntry::Loaded(d) = existing_thumbnail.lock().unwrap().deref() {
                                post_request_retval = (
                                    MediaCacheEntry::Loaded(Arc::clone(d)),
                                    MediaFormat::Thumbnail(existing_mts.clone()),
                                );
                            }
                        }
                        entry_ref
                    }
                }
            }
        };

        sliding_sync::submit_async_request(
            MatrixRequest::FetchMedia {
                media_request: MediaRequestParameters {
                    source: MediaSource::Plain(mxc_uri),
                    format: requested_format,
                },
                on_fetched: insert_into_cache,
                destination: entry_ref,
                update_sender: self.timeline_update_sender.clone(),
            }
        );
        post_request_retval
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
        Err(e) => match e {
            Error::Http(http_error) => {
                if let Some(client_error) = http_error.as_client_api_error() {
                    error!("Client error for media cache: {client_error} request: {:?}", request);
                    MediaCacheEntry::Failed(client_error.clone())
                } else {
                    match *http_error {
                        HttpError::Reqwest(reqwest_error) => {
                            if !reqwest_error.is_connect() {
                                MediaCacheEntry::Failed(ClientError::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    ErrorBody::Json(json!({})),
                                ))
                            } else if reqwest_error.is_timeout() {
                                MediaCacheEntry::Failed(ClientError::new(
                                    StatusCode::REQUEST_TIMEOUT,
                                    ErrorBody::Json(json!({})),
                                ))
                            } else if reqwest_error.is_status() {
                                MediaCacheEntry::Failed(ClientError::new(
                                    reqwest_error
                                        .status()
                                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                                    ErrorBody::Json(json!({})),
                                ))
                            } else {
                                MediaCacheEntry::Failed(ClientError::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    ErrorBody::Json(json!({})),
                                ))
                            }
                        }
                        _ => MediaCacheEntry::Failed(ClientError::new(
                            StatusCode::NOT_FOUND,
                            ErrorBody::Json(json!({})),
                        )),
                    }
                }
            }
            Error::InsufficientData => MediaCacheEntry::Failed(ClientError::new(
                StatusCode::PARTIAL_CONTENT, 
                ErrorBody::Json(json!({}))
            )),
            Error::AuthenticationRequired => MediaCacheEntry::Failed(ClientError::new(
                StatusCode::UNAUTHORIZED, 
                ErrorBody::Json(json!({}))
            )),
            _ => MediaCacheEntry::Failed(ClientError::new(
                StatusCode::INTERNAL_SERVER_ERROR, ErrorBody::Json(json!({}))
            ))
        }
    };

    *value_ref.lock().unwrap() = new_value;

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched(Some(request.clone())));
    }
    SignalToUI::set_ui_signal();
}
