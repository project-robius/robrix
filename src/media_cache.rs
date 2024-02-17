use std::{sync::{Mutex, Arc}, collections::{BTreeMap, btree_map::Entry}, time::SystemTime, ops::{Deref, DerefMut}};
use makepad_widgets::{error, log};
use matrix_sdk::{ruma::{OwnedMxcUri, events::room::MediaSource}, media::{MediaRequest, MediaFormat}};
use crate::{sliding_sync::{self, MatrixRequest}, utils::{MEDIA_THUMBNAIL_FORMAT, MediaFormatConst}};

pub static AVATAR_CACHE: Mutex<MediaCache> = Mutex::new(MediaCache::new(MEDIA_THUMBNAIL_FORMAT));

pub type MediaCacheEntryRef = Arc<Mutex<MediaCacheEntry>>;

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
    pub const fn new(default_format: MediaFormatConst) -> Self {
        Self {
            cache: BTreeMap::new(),
            default_format,
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
            }
        );
        MediaCacheEntry::Requested
    }

}

/// Insert data into a previously-requested media cache entry.
fn insert_into_cache(value_ref: &Mutex<MediaCacheEntry>, _request: MediaRequest, data: matrix_sdk::Result<Vec<u8>>) {
    let new_value = match data {
        Ok(data) => {
            
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

            MediaCacheEntry::Loaded(data.into())
        }
        Err(e) => {
            error!("Failed to fetch media for {:?}: {e:?}", _request.source);
            MediaCacheEntry::Failed
        }
    };
    *value_ref.lock().unwrap() = new_value;
}
