use std::{sync::{Mutex, Arc}, collections::BTreeMap, time::SystemTime};

use matrix_sdk::{ruma::{OwnedMxcUri, events::room::MediaSource}, media::MediaRequest};

use crate::sliding_sync::{self, MatrixRequest};

/// An entry in the avatar cache.
enum AvatarCacheEntry {
    /// A request has been issued and we're waiting for it to complete.
    Requested,
    /// The avatar has been successfully loaded from the server.
    Loaded(Arc<[u8]>),
    /// The avatar failed to load from the server.
    Failed,
}
impl AvatarCacheEntry {
    fn to_option(&self) -> Option<Arc<[u8]>> {
        match self {
            AvatarCacheEntry::Loaded(data) => Some(data.clone()),
            _ => None,
        }
    }
}

static AVATAR_CACHE: Mutex<BTreeMap<OwnedMxcUri, AvatarCacheEntry>> = Mutex::new(BTreeMap::new());

/// Quickly try to fetch an avatar from the cache.
///
/// This is suitable for use in a latency-sensitive context, such as a UI draw routine.
pub fn try_get_avatar(mxc_uri: &OwnedMxcUri) -> Option<Arc<[u8]>> {
    AVATAR_CACHE.lock().unwrap()
        .get(mxc_uri)
        .and_then(AvatarCacheEntry::to_option)
}

/// Tries to get the avatar from the cache, or submits an async request to fetch it.
///
/// This method *does not* block or wait for the avatar to be fetched,
/// and will return `None` while the async request is in flight.
/// If a request is already in flight, this will return `None` and not issue a new redundant request.
pub fn try_get_avatar_or_fetch(mxc_uri: &OwnedMxcUri) -> Option<Arc<[u8]>> {
    match AVATAR_CACHE.lock().unwrap().get(mxc_uri) {
        Some(AvatarCacheEntry::Requested) => return None,
        Some(AvatarCacheEntry::Loaded(data)) => return Some(Arc::clone(&data)),
        Some(AvatarCacheEntry::Failed) => return None,
        None => { }, // fall through to send a request.
    }

    let mxc_uri2 = mxc_uri.clone();
    sliding_sync::submit_async_request(MatrixRequest::FetchMedia {
        media_request: MediaRequest {
            source: MediaSource::Plain(mxc_uri.clone()),
            format: sliding_sync::media_thumbnail_format(),
        },
        on_fetched: Box::new(move |media_result| insert_into_cache(mxc_uri2, media_result)),
    });
    AVATAR_CACHE.lock().unwrap().insert(mxc_uri.clone(), AvatarCacheEntry::Requested);

    None
}

/// Insert data into the avatar cache.
fn insert_into_cache(mxc_uri: OwnedMxcUri, data: matrix_sdk::Result<Vec<u8>>) {
    AVATAR_CACHE.lock().unwrap().insert(
        mxc_uri.clone(),
        match data {
            Ok(data) => {
                
                // debugging: dump out the avatar image to disk
                if false {
                    println!("Fetched media for {mxc_uri}");
                    let mut path = crate::temp_storage::get_temp_dir_path().clone();
                    let filename = format!("{}_{}_{}",
                        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
                        mxc_uri.server_name().unwrap(), mxc_uri.media_id().unwrap(),
                    );
                    path.push(filename);
                    path.set_extension("png");
                    println!("Writing user avatar image to disk: {:?}", path);
                    std::fs::write(path, &data)
                        .expect("Failed to write user avatar image to disk");
                }

                AvatarCacheEntry::Loaded(data.into())
            }
            Err(e) => {
                eprintln!("Failed to fetch media at {mxc_uri}: {e:?}");
                AvatarCacheEntry::Failed
            }
        }
    );
}
