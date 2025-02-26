use core::panic;
use std::{sync::{Mutex, Arc}, collections::{BTreeMap, btree_map::Entry}, time::SystemTime, ops::{Deref, DerefMut}};
use makepad_widgets::{error, log, SignalToUI};
use matrix_sdk::{ruma::{OwnedMxcUri, events::room::MediaSource}, media::{MediaRequest, MediaFormat}};
use crate::{home::room_screen::TimelineUpdate, sliding_sync::{self, MatrixRequest}};

pub type EntryAndFormatRef = Arc<Mutex<EntryAndFormat>>;
pub type Caches = Vec<EntryAndFormatRef>;

#[derive(Debug, Clone)]
pub struct EntryAndFormat {
    pub entry: MediaCacheEntry,
    pub format: MediaFormat,
}

impl EntryAndFormat {
    pub const fn new(entry: MediaCacheEntry, format: MediaFormat) -> Self {
        Self {entry, format}
    }
}
/// An entry in the media cache.
#[derive(Debug, Clone)]
pub enum MediaCacheEntry {
    /// A request has been issued and we're waiting for it to complete.
    Requested,
    /// The media has been successfully loaded from the server.
    Loaded(Arc<[u8]>),
    /// The media failed to load from the server.
    Failed,
}

/// A cache of fetched media. Keys are Matrix URIs, values are references to byte arrays.
pub struct MediaCache {
    /// The actual cached data.
    cache: BTreeMap<OwnedMxcUri, Caches>,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}
impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, Caches>;
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

    /// Gets media from the cache without sending a fetch request if the media is absent.
    ///
    /// This is suitable for use in a latency-sensitive context, such as a UI draw routine.
    pub fn try_get_original_media(&self, mxc_uri: &OwnedMxcUri) -> Option<MediaCacheEntry> {
        self.get(mxc_uri).and_then(|v|{
            for entry_and_format in v {
                let mutex_guard = entry_and_format.lock().unwrap();
                let entry_and_format = mutex_guard.deref().clone();

                if matches!(&entry_and_format.format, &MediaFormat::File) {
                    return Some(entry_and_format.entry.clone());
                } else {
                    continue;
                }
            }
            None
        })
    }

    pub fn try_get_thumbnail_media(&self, mxc_uri: &OwnedMxcUri) -> Option<MediaCacheEntry> {
        self.get(mxc_uri).and_then(|v|{
            for entry_and_format in v {
                let mutex_guard = entry_and_format.lock().unwrap();
                let entry_and_format = mutex_guard.deref().clone();
                if !matches!(&entry_and_format.format, &MediaFormat::File) {
                    return Some(entry_and_format.entry.clone());
                } else {
                    continue;
                }
            }
            None
        })
    }

    /// Tries to get the media from the cache, or submits an async request to fetch it.
    ///
    /// This method *does not* block or wait for the media to be fetched,
    /// and will return `MediaCache::Requested` while the async request is in flight.
    /// If a request is already in flight, this will not issue a new redundant request.
    pub fn try_get_thumbnail_media_or_fetch(
        &mut self,
        mxc_uri: OwnedMxcUri,
        thumbnail_format: MediaFormat,
    ) -> MediaCacheEntry {
        if matches!(thumbnail_format.clone(), MediaFormat::File) {
            panic!("MediaFormat parameter Error")
        }

        let value_ref = match self.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                let entry_and_format = EntryAndFormat::new(
                    MediaCacheEntry::Requested,
                    thumbnail_format.clone()
                );
                vacant.insert(vec![Arc::new(Mutex::new(entry_and_format))])[0].clone()
            },
            Entry::Occupied(mut occupied) => {
                for entry_and_format in occupied.get() {
                    let mutex_guard = entry_and_format.lock().unwrap();
                    let entry_and_format = mutex_guard.deref();
                    if !matches!(entry_and_format.format, MediaFormat::File) {
                        return entry_and_format.entry.clone();
                    }
                    continue;
                }
                let entry_and_format = EntryAndFormat::new(MediaCacheEntry::Requested, thumbnail_format.clone());
                occupied.get_mut().push(Arc::new(Mutex::new(entry_and_format)));
                occupied.get().last().unwrap().clone()
            },
        };

        let destination = value_ref.clone();

        sliding_sync::submit_async_request(
            MatrixRequest::FetchMedia {
                media_request: MediaRequest {
                    source: MediaSource::Plain(mxc_uri),
                    format: thumbnail_format,
                },
                on_fetched: insert_into_cache,
                destination,
                update_sender: self.timeline_update_sender.clone(),
            }
        );
        MediaCacheEntry::Requested
    }

    pub fn try_get_original_media_or_fetch(
        &mut self,
        mxc_uri: OwnedMxcUri,
    ) -> MediaCacheEntry {
        let value_ref = match self.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                let entry_and_format = EntryAndFormat::new(
                    MediaCacheEntry::Requested,
                    MediaFormat::File
                );
                vacant.insert(vec![Arc::new(Mutex::new(entry_and_format))])[0].clone()
            },
            Entry::Occupied(mut occupied) => {
                 for entry_and_format in occupied.get() {
                    let mutex_guard = entry_and_format.lock().unwrap();
                    let entry_and_format = mutex_guard.deref();
                    if matches!(entry_and_format.format, MediaFormat::File) {
                        return entry_and_format.entry.clone();
                    }
                    continue;
                }
                let entry_and_format = EntryAndFormat::new(MediaCacheEntry::Requested, MediaFormat::File);
                occupied.get_mut().push(Arc::new(Mutex::new(entry_and_format)));
                occupied.get().last().unwrap().clone()
            },
        };

        let destination = value_ref.clone();

        sliding_sync::submit_async_request(
            MatrixRequest::FetchMedia {
                media_request: MediaRequest {
                    source: MediaSource::Plain(mxc_uri),
                    format: MediaFormat::File,
                },
                on_fetched: insert_into_cache,
                destination,
                update_sender: self.timeline_update_sender.clone(),
            }
        );
        MediaCacheEntry::Requested
    }
}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache<D: Into<Arc<[u8]>>>(
    value_ref: &Mutex<EntryAndFormat>,
    request: MediaRequest,
    data: matrix_sdk::Result<D>,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let format = request.format.clone();
    let entry = match data {
        Ok(data) => {
            let data = data.into();

            // debugging: dump out the media image to disk
            if false {
                if let MediaSource::Plain(mxc_uri) = request.source {
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
            error!("Failed to fetch media for {:?}: {e:?}", request.source);
            MediaCacheEntry::Failed
        }
    };

    *value_ref.lock().unwrap() = EntryAndFormat::new(entry, format);

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched);
    }
    SignalToUI::set_ui_signal();
}
