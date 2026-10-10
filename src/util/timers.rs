use std::{thread, time::Duration};

pub fn set_timeout<F>(duration: Duration, callback: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(move || {
        thread::sleep(duration);
        callback();
    });
}
