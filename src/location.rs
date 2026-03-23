//! Functions for querying the device's current location.

use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    time::SystemTime,
};

use makepad_widgets::{Cx, error, log};
use robius_location::{Access, Accuracy, Coordinates, Location, Manager};

/// The action emitted upon every location update.
#[derive(Copy, Clone, Debug)]
pub enum LocationAction {
    /// The location handler received a new location update.
    Update(LocationUpdate),
    /// The location handler encountered an error.
    Error(robius_location::Error),
    None,
}

/// An updated location sample, including coordinates and a system timestamp.
#[derive(Copy, Clone, Debug)]
pub struct LocationUpdate {
    pub coordinates: Coordinates,
    pub time: Option<SystemTime>,
}

static LATEST_LOCATION: Mutex<Option<LocationUpdate>> = Mutex::new(None);

/// Returns the latest location update's coordinates, if available.
///
/// Note that this function is guaranteed to return `None` if
/// [`init_location_subscriber`] has not been called yet.
pub fn get_latest_location() -> Option<LocationUpdate> {
    *(LATEST_LOCATION.lock().unwrap())
}

struct LocationHandler;

impl robius_location::Handler for LocationHandler {
    fn handle(&self, location: Location<'_>) {
        let coords = location.coordinates();
        log!("Received location update: {coords:?}");
        match coords {
            Ok(coords) => {
                let update = LocationUpdate {
                    coordinates: coords,
                    time: location.time().ok(),
                };
                Cx::post_action(LocationAction::Update(update));
                *LATEST_LOCATION.lock().unwrap() = Some(update);
            }
            Err(e) => {
                error!("Error getting coordinates from location update: {e:?}");
                Cx::post_action(LocationAction::Error(e));
            }
        }
    }

    fn error(&self, e: robius_location::Error) {
        error!("Got error in location handler: {e:?}");
        Cx::post_action(LocationAction::Error(e));
    }
}

fn location_request_loop(
    request_receiver: Receiver<LocationRequest>,
    mut manager: ManagerWrapper,
) -> Result<(), robius_location::Error> {
    manager.update_once()?;

    while let Ok(request) = request_receiver.recv() {
        match request {
            LocationRequest::UpdateOnce => {
                manager.update_once()?;
            }
            LocationRequest::StartUpdates => {
                manager.start_updates()?;
            }
            LocationRequest::StopUpdates => {
                manager.stop_updates()?;
            }
        }
    }

    error!("Location request loop exited unexpectedly (the senders all died).");
    Err(robius_location::Error::Unknown)
}

pub enum LocationRequest {
    UpdateOnce,
    StartUpdates,
    StopUpdates,
}

static LOCATION_REQUEST_SENDER: Mutex<Option<Sender<LocationRequest>>> = Mutex::new(None);

/// Submits a request to start, stop, or get a single new location update(s).
pub fn request_location_update(request: LocationRequest) {
    if let Some(sender) = LOCATION_REQUEST_SENDER.lock().unwrap().as_ref() {
        if let Err(err) = sender.send(request) {
            error!("Error sending location request: {err:?}");
        }
    } else {
        error!("No location request sender available.");
    }
}

/// Spawns a thread to listen for location requests and updates to the latest location.
///
/// This will request a single location update immediately upon starting.
/// To request additional updates, use [`request_location_update`].
///
/// It is okay to call this function multiple times, as it will only re-initialize
/// the location subscriber thread if it has not been initialized yet
/// or if it has died and needs to be restarted.
///
/// This function requires passing in a reference to `Cx`,
/// which isn't used, but acts as a guarantee that this function
/// must only be called by the main UI thread.
pub fn init_location_subscriber(_cx: &mut Cx) -> Result<(), robius_location::Error> {
    let mut lrs = LOCATION_REQUEST_SENDER.lock().unwrap();
    if lrs.is_some() {
        log!("Location subscriber already initialized.");
        return Ok(());
    }
    let manager = ManagerWrapper(Manager::new(LocationHandler)?);
    manager.request_authorization(Access::Foreground, Accuracy::Precise)?;
    let _ = manager.update_once();

    let (request_sender, request_receiver) = mpsc::channel::<LocationRequest>();
    *lrs = Some(request_sender);
    std::thread::spawn(|| location_request_loop(request_receiver, manager));
    Ok(())
}

struct ManagerWrapper(Manager);
unsafe impl Send for ManagerWrapper {}
unsafe impl Sync for ManagerWrapper {}
impl std::ops::Deref for ManagerWrapper {
    type Target = Manager;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ManagerWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
