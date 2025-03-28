use std::{sync::{Mutex, Arc}, collections::{BTreeMap, btree_map::Entry}, time::SystemTime, ops::{Deref, DerefMut}};
use makepad_widgets::{error, log, SignalToUI};
use matrix_sdk::{media::{MediaFormat, MediaRequestParameters}, ruma::{events::room::MediaSource, OwnedMxcUri}};
use crate::{home::room_screen::TimelineUpdate, sliding_sync::{self, MatrixRequest}, utils::MediaFormatConst};

pub type MediaCacheEntryRef = Arc<Mutex<EntryAndFormat>>;

/// We want all the `MediaCacheEntryRef` stored in heap.
pub type ValuesVec = SmallVec<[MediaCacheEntryRef; 0]>;

#[derive(Debug, Clone)]
pub struct EntryAndFormat {
    pub entry: MediaCacheEntry,
    pub format: MediaFormat,
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
    cache: BTreeMap<OwnedMxcUri, ValuesVec>,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}
impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, ValuesVec>;
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
    pub fn try_get_media(&self, mxc_uri: &OwnedMxcUri, prefer_thumbnail: bool) -> MediaCacheEntry {
        self.get(mxc_uri).map_or(MediaCacheEntry::Failed, |entries| {
            let matches_preferred = |e: &EntryAndFormat| {
                prefer_thumbnail != matches!(e.format, MediaFormat::File)
            };

            entries.iter()
                .map(|e| e.lock().unwrap().clone())
                .find(matches_preferred)
                .map_or_else(
                    || entries.iter()
                        .map(|e| e.lock().unwrap().clone())
                        .find(|e| matches!(e.entry, MediaCacheEntry::Loaded(..)))
                        .map_or(MediaCacheEntry::Requested, |e| e.entry),
                    |e| e.entry
                )
        })
    }

    /// Tries to get the media from the cache, or submits an async request to fetch it.
    ///
    /// This method *does not* block or wait for the media to be fetched,
    /// and will return `MediaCache::Requested` while the async request is in flight.
    /// If a request is already in flight, this will not issue a new redundant request.
    pub fn try_get_media_or_fetch(
        &mut self,
        mxc_uri: OwnedMxcUri,
        media_format: Option<MediaFormat>,
        prefer_thumbnail: bool,
    ) -> MediaCacheEntry {

        let media_format = media_format.unwrap_or(MediaFormat::File);
        let value_ref = match self.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => {
                let entry_and_format_ref = Arc::new(Mutex::new(EntryAndFormat {
                    entry: MediaCacheEntry::Requested,
                    format: media_format.clone()
                }));

                // note we just insert the first value into the cache so we can get [0].
                vacant.insert(smallvec![entry_and_format_ref.clone()]);
                entry_and_format_ref
            },
            Entry::Occupied(mut occupied) => {
                for entry_and_format in occupied.get() {
                    let mutex_guard = entry_and_format.lock().unwrap();
                    let entry_and_format = mutex_guard.deref();

                    // anti condition
                    if prefer_thumbnail && !matches!(entry_and_format.format, MediaFormat::File)
                        ||
                    !prefer_thumbnail && matches!(entry_and_format.format, MediaFormat::File)
                    {
                        return entry_and_format.entry.clone();
                    }
                }

                // if not found, we push a new value and return its ref.
                let entry_and_format = Arc::new(Mutex::new(EntryAndFormat {
                    entry: MediaCacheEntry::Requested,
                    format: media_format.clone()
                }));
                occupied.get_mut().push(entry_and_format.clone());
                entry_and_format
            },
        };

        let destination = value_ref.clone();

        sliding_sync::submit_async_request(
            MatrixRequest::FetchMedia {
                media_request: MediaRequestParameters {
                    source: MediaSource::Plain(mxc_uri),
                    format: media_format,
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
    value_ref: &Mutex<MediaCacheEntry>,
    _request: MediaRequestParameters,
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

    *value_ref.lock().unwrap() = EntryAndFormat {entry, format};

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::MediaFetched);
    }
    SignalToUI::set_ui_signal();
}
