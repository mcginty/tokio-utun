extern crate futures;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_utun;

use futures::Stream;
use tokio::prelude::*;
use tokio_codec::BytesCodec;
use tokio_utun::UtunStream;
use std::time::{Duration, Instant};

// NOTE: this test should only be run via the test.sh wrapper script
#[test]
fn smoke() {
    let utun = UtunStream::connect("utun5").unwrap();
    let utun = utun.framed(BytesCodec::new());
    let deadline = Instant::now() + Duration::from_secs(5);

    tokio::run(utun
        .map_err(|_| println!("utun error!"))
        .take(1)
        .for_each(|_| { println!("got a packet"); Ok(()) })
        .deadline(deadline)
        .map_err(|_| println!("deadline passed!"))
    );
}