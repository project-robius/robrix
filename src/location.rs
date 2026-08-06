//! Functions for querying the device's current location.

use std::{sync::Mutex, time::SystemTime};

use makepad_widgets::{Cx, error, log};
use robius_location::{Access, Accuracy, Coordinates, Location, Manager};

/// The action emitted upon every location update.
#[derive(Copy, Clone, Debug)]
pub enum LocationAction {
    /// The location handler received a new location update.
    Update(LocationUpdate),
    /// The location handler encountered an error.
    Error(robius_location::Error),
}

/// An updated location sample, including coordinates and a system timestamp.
#[derive(Copy, Clone, Debug)]
pub struct LocationUpdate {
    pub coordinates: Coordinates,
    pub time: Option<SystemTime>,
}

static LATEST_LOCATION: Mutex<Option<LocationUpdate>> = Mutex::new(None);

/// Returns the latest location update, if one has been received.
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


pub enum LocationRequest {
    UpdateOnce,
    StartUpdates,
    StopUpdates,
}

/// A wrapper struct for storing the singleton location manager in Cx globals.
#[derive(Default)]
struct LocationManagerGlobal(Option<Manager>);

/// Submits a request to start, stop, or get a single new location update(s).
pub fn request_location_update(cx: &mut Cx, request: LocationRequest) {
    let Some(manager) = cx.global::<LocationManagerGlobal>().0.as_mut() else {
        error!("Location subscriber not initialized on this thread.");
        Cx::post_action(LocationAction::Error(robius_location::Error::Unknown));
        return;
    };
    let (result, show_error) = match request {
        LocationRequest::UpdateOnce => (manager.update_once(), true),
        LocationRequest::StartUpdates => (manager.start_updates(), true),
        LocationRequest::StopUpdates => (manager.stop_updates(), false),
    };
    if let Err(e) = result {
        error!("Error handling location request: {e:?}");
        if show_error {
            Cx::post_action(LocationAction::Error(e));
        }
    }
}

/// Initializes the location manager and requests a single location update.
///
/// To request additional updates, use [`request_location_update`].
///
/// It is okay to call this function multiple times; it only initializes the manager once.
pub fn init_location_subscriber(cx: &mut Cx) -> Result<(), robius_location::Error> {
    let lm = cx.global::<LocationManagerGlobal>();
    if lm.0.is_some() {
        // log!("Location subscriber already initialized.");
        return Ok(());
    }

    let new_manager = Manager::new(LocationHandler)?;
    new_manager.request_authorization(Access::Foreground, Accuracy::Precise)?;
    let _ = new_manager.update_once();
    lm.0 = Some(new_manager);
    Ok(())
}
