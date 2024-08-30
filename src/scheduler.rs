use lazy_static::lazy_static;
use std::sync::{Arc,Mutex};
use std::collections::HashMap;
use std::time::{Instant,Duration};
use std::thread;

pub struct Schedule{
    job:  Box<dyn Fn() +Send + Sync>,
    start_time:Instant,
    wait_time:Duration,
    body:String,
}
lazy_static!{
    static ref SCHEDULER: Arc<Mutex<HashMap<String,Schedule>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref APP_FOCUS:Mutex<bool> = Mutex::new(true);
}
pub fn add_job(id:String,body:String,wait_time:Duration,job:Box<dyn Fn() +Send + Sync>){
    let schedule = Schedule{
        job,
        start_time:Instant::now(),
        wait_time,
        body
    };
    SCHEDULER.lock().unwrap().insert(id, schedule);
}
pub fn init(){
    let schedule_c = SCHEDULER.clone();
    thread::spawn(move ||{
        loop{
            let mut schedulers = schedule_c.lock().unwrap();
            let mut to_remove: Vec<String> = vec![];
            let app_focus: bool =  *APP_FOCUS.lock().unwrap();
            for (id,sch) in schedulers.iter_mut(){
                if app_focus{
                    if sch.start_time.elapsed() >= sch.wait_time {
                        let t = &sch.job;
                        t();
                        to_remove.push(id.clone());
                    }
                }else{
                    // when app loses focus, the schedule's start time is reset to current time
                    sch.start_time = Instant::now();
                }
            }
            for id in to_remove{
                schedulers.remove(&id);
            }
            thread::sleep(Duration::new(2,0));
        }
    });
}