use std::{sync::{Mutex, Arc}, collections::{BTreeMap, btree_map::Entry}, time::SystemTime, ops::{Deref, DerefMut}};
use makepad_widgets::{error, log};
use matrix_sdk::{ruma::{OwnedMxcUri, events::room::MediaSource}, media::{MediaRequest, MediaFormat}};
use crate::{home::room_screen::TimelineUpdate, sliding_sync::{self, MatrixRequest}, utils::{MediaFormatConst, MEDIA_THUMBNAIL_FORMAT}};

pub type MediaCacheEntryRef = Arc<Mutex<MediaCacheEntry>>;

pub static AVATAR_CACHE: MediaCacheLocked = MediaCacheLocked(Mutex::new(MediaCache::new(MEDIA_THUMBNAIL_FORMAT, None)));

pub struct MediaCacheLocked(Mutex<MediaCache>);
impl Deref for MediaCacheLocked {
    type Target = Mutex<MediaCache>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl MediaCacheLocked {
    /// Similar to [`Self::try_get_media_or_fetch()`], but immediately fires off an async request
    /// on the current task to fetch the media, blocking until the request completes.
    ///
    /// Unlike other functions, this is intended for use in background tasks or other async contexts
    /// where it is not latency-sensitive, and safe to block on the async request.
    /// Thus, it must be implemented on the `MediaCacheLocked` type, which is safe to hold a reference to
    /// across an await point, whereas a mutable reference to a locked `MediaCache` is not (i.e., a `MutexGuard`).
    pub async fn get_media_or_fetch_async(
        &self,
        client: &matrix_sdk::Client,
        mxc_uri: OwnedMxcUri,
        media_format: Option<MediaFormat>,
    ) -> Option<Arc<[u8]>> {
        let destination = {
            match self.lock().unwrap().entry(mxc_uri.clone()) {
                Entry::Vacant(vacant) => vacant
                    .insert(Arc::new(Mutex::new(MediaCacheEntry::Requested)))
                    .clone(),
                Entry::Occupied(occupied) => match occupied.get().lock().unwrap().deref() {
                    MediaCacheEntry::Loaded(data) => return Some(data.clone()),
                    MediaCacheEntry::Failed => return None,
                    // If already requested (a fetch is in process),
                    // we return None for now and allow the `insert_into_cache` function
                    // emit a UI Signal when the fetch completes,
                    // which will trigger a re-draw of the UI,
                    // and thus a re-fetch of any visible avatars.
                    MediaCacheEntry::Requested => return None,
                }
            }
        };

        let media_request = MediaRequest {
            source: MediaSource::Plain(mxc_uri),
            format: media_format.unwrap_or_else(|| self.lock().unwrap().default_format.clone().into()),
        };

        let res = client
            .media()
            .get_media_content(&media_request, true)
            .await;
        let res: matrix_sdk::Result<Arc<[u8]>> = res.map(|d| d.into());
        let retval = res
            .as_ref()
            .ok()
            .cloned();
        insert_into_cache(
            &destination,
            media_request,
            res,
            None,
        );
        retval
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
    cache: BTreeMap<OwnedMxcUri, MediaCacheEntryRef>,
    /// The default format to use when fetching media.
    default_format: MediaFormatConst,
    /// A channel to send updates to a particular timeline when a media request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}
impl Deref for MediaCache {
    type Target = BTreeMap<OwnedMxcUri, MediaCacheEntryRef>;
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
        default_format: MediaFormatConst,
        timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            default_format,
            timeline_update_sender,
        }
    }

    /// Gets media from the cache without sending a fetch request if the media is absent.
    ///
    /// This is suitable for use in a latency-sensitive context, such as a UI draw routine.
    pub fn try_get_media(&self, mxc_uri: &OwnedMxcUri) -> Option<MediaCacheEntry> {
        self.get(mxc_uri).map(|v| v.lock().unwrap().deref().clone())
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
    ) -> MediaCacheEntry {
        let value_ref = match self.entry(mxc_uri.clone()) {
            Entry::Vacant(vacant) => vacant.insert(
                Arc::new(Mutex::new(MediaCacheEntry::Requested))
            ),
            Entry::Occupied(occupied) => return occupied.get().lock().unwrap().deref().clone(),
        };

        let destination = Arc::clone(value_ref);
        let format = media_format.unwrap_or_else(||
            self.default_format.clone().into()
        );
        sliding_sync::submit_async_request(
            MatrixRequest::FetchMedia {
                media_request: MediaRequest {
                    source: MediaSource::Plain(mxc_uri),
                    format,
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
    _request: MediaRequest,
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
}
