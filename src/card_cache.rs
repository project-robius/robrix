use std::{
    sync::{Mutex, Arc},
    cell::RefCell, 
    collections::{BTreeMap, btree_map::Entry},
    ops::{Deref, DerefMut}
};
use makepad_widgets::{error, SignalToUI};

use crate::{
    home::room_screen::TimelineUpdate,
    sliding_sync::{self, MatrixRequest},
};

use url_preview;

pub type LinkPreviewResult = Result<LinkPreviewCard, url_preview::PreviewError>;

// Link preview card data
#[derive(Clone, Debug)]
pub struct LinkPreviewCard {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub raw_content: String,
    pub image: Option<Arc<Vec<u8>>>
}

/// An entry in the avatar cache.
#[derive(Clone, Debug)]
pub enum CardCacheEntry {
    Loaded(Arc<LinkPreviewCard>),
    Requested,
    Failed,
}

pub type CardCacheEntryRef = Arc<Mutex<CardCacheEntry>>;

thread_local! {
    /// A cache of LinkPreviewCard, indexed by url.
    ///
    /// To be of any use, this cache must only be accessed by the main UI thread.
    static CARD_CACHE: RefCell<BTreeMap<String, CardCacheEntry>> = const { RefCell::new(BTreeMap::new()) };
}


/// A cache of fetched card. Keys are url, values are references to byte arrays.
pub struct CardCache {
    /// The actual cached data.
    cache: BTreeMap<String, CardCacheEntryRef>,
    /// A channel to send updates to a particular timeline when a request has completed.
    timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
}
impl Deref for CardCache {
    type Target = BTreeMap<String, CardCacheEntryRef>;
    fn deref(&self) -> &Self::Target {
        &self.cache
    }
}
impl DerefMut for CardCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cache
    }
}

impl CardCache {
    /// Creates a new LinkPreviewCard cache that will use the given url
    /// when fetching data from the server.
    ///
    /// It will also optionally send updates to the given timeline update sender
    /// when a request has completed.
    pub const fn new(
        timeline_update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
    ) -> Self {
        Self {
            cache: BTreeMap::new(),
            timeline_update_sender,
        }
    }

    pub fn try_get_card(&self, url: String) -> Option<CardCacheEntry> {
        self.get(&url).map(|v| v.lock().unwrap().deref().clone())
    }

    /// Tries to get the LinkPreviewCard from the cache, or submits an async request to fetch it.
    ///
    /// This method *does not* block or wait for the media to be fetched,
    /// and will return `CardCache::Requested` while the async request is in flight.
    /// If a request is already in flight, this will not issue a new redundant request.
    pub fn try_get_card_or_fetch(
        &mut self,
        url: String,
        raw_content: String,
    ) -> CardCacheEntry {
        let value_ref = match self.entry(url.clone()) {
            Entry::Vacant(vacant) => vacant.insert(
                Arc::new(Mutex::new(CardCacheEntry::Requested))
            ),
            Entry::Occupied(occupied) => return occupied.get().lock().unwrap().deref().clone(),
        };

        let destination = Arc::clone(value_ref);
        sliding_sync::submit_async_request(
            MatrixRequest::FetchLinkPreviewCard {
                url,
                raw_content,
                on_fetched: insert_into_cache,
                destination,
                update_sender: self.timeline_update_sender.clone(),
            }
        );
        CardCacheEntry::Requested
    }
}

/// Insert data into a previously-requested card cache entry.
fn insert_into_cache(
    value_ref: &Mutex<CardCacheEntry>,
    url: String,
    data: LinkPreviewResult,
    update_sender: Option<crossbeam_channel::Sender<TimelineUpdate>>,
) {
    let new_value = match data {
        Ok(data) => {
            let data = data.into();
            CardCacheEntry::Loaded(data)
        }
        Err(e) => {
            CardCacheEntry::Failed
        }
    };
    *value_ref.lock().unwrap() = new_value;

    if let Some(sender) = update_sender {
        let _ = sender.send(TimelineUpdate::LinkPreviewCardFetched);
    }
    SignalToUI::set_ui_signal();
}

