use lazy_static::lazy_static;
use std::sync::{Arc,Mutex};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;
use matrix_sdk::ruma::{
    MilliSecondsSinceUnixEpoch, OwnedEventId,OwnedRoomId
};
const FULLY_READ_FRAME_DURATION: u64 = 5;
lazy_static!{
    // To-do: Use App_Focus to eliminate fully read events when App loses focus during 5 seconds time frame
    pub static ref APP_FOCUS:Mutex<bool> = Mutex::new(true);
    pub static ref READ_EVENT_HASHMAP : Arc<Mutex<HashMap<String,(OwnedRoomId,OwnedEventId,MilliSecondsSinceUnixEpoch,std::time::Instant)>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref MARKED_FULLY_READ_QUEUE:Arc<Mutex<HashMap<String,(OwnedRoomId,OwnedEventId,MilliSecondsSinceUnixEpoch)>>> = Arc::new(Mutex::new(HashMap::new()));
}

// spawn a background thread to handle fully_read event when the message is displayed on the screen for at least 5 seconds
pub fn init(){
    let read_event_hashmap_c = READ_EVENT_HASHMAP.clone();
    let marked_fully_read_queue_c = MARKED_FULLY_READ_QUEUE.clone();
    thread::spawn(move ||{
        loop{
            let mut to_remove = vec![];
            let mut read_event_hashmap = read_event_hashmap_c.lock().unwrap();
            for (key,(room_id,event_id,timestamp,timing)) in read_event_hashmap.iter(){
                if timing.elapsed() > Duration::from_secs(FULLY_READ_FRAME_DURATION){
                    marked_fully_read_queue_c.lock().unwrap().insert(event_id.to_string(),(room_id.clone(),event_id.clone(),timestamp.clone()));
                    to_remove.push(key.clone());
                }
            }
            for to_remove in to_remove{
                read_event_hashmap.remove(&to_remove);
            }
            drop(read_event_hashmap);
            thread::sleep(Duration::new(2,0));
        }
    });
}