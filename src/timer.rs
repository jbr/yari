use std::ops::Range;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use rand::{thread_rng, Rng};
use rand::distributions::{Distribution, Uniform};

pub enum TimerEvent {
    Expiry,
    Extension
}

pub fn start<F>(
    interval_range_ms: Range<u32>,
    expired: F,
) -> Sender<TimerEvent>
where
    F: Fn() -> (),
{
    let (tx, rx) = channel::<TimerEvent>();
    let distribution: Uniform<_> = interval_range_ms.into();

    let transmit = tx.clone();
    
    thread::spawn(move || {
        let mut rng = thread_rng();
        let sleep_time = distribution.sample(&mut rng);

        match rx.recv_timeout(Duration::from_millis(sleep_time.into())) {
            Ok(_) =>  (),
            Err(_) => {
                expired();
            }
        }
    });

    transmit
}
