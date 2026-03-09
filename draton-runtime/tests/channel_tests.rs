use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use draton_runtime::scheduler::channel::Chan;

#[test]
fn send_and_recv_basic() {
    let chan = Chan::new(1);
    chan.send(42);
    assert_eq!(chan.recv(), 42);
}

#[test]
fn blocked_sender_unblocks_after_recv() {
    let chan = Arc::new(Chan::new(1));
    chan.send(1);
    let sender = {
        let chan = Arc::clone(&chan);
        thread::spawn(move || chan.send(2))
    };
    thread::sleep(Duration::from_millis(10));
    assert_eq!(chan.recv(), 1);
    sender.join().expect("sender thread");
    assert_eq!(chan.recv(), 2);
}

#[test]
fn blocked_receiver_unblocks_after_send() {
    let chan = Arc::new(Chan::new(1));
    let recv = {
        let chan = Arc::clone(&chan);
        thread::spawn(move || chan.recv())
    };
    thread::sleep(Duration::from_millis(10));
    chan.send(99);
    assert_eq!(recv.join().expect("receiver thread"), 99);
}

#[test]
fn multiple_senders_deliver_in_fifo_order() {
    let chan = Arc::new(Chan::new(4));
    let turn = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for (index, value) in [1, 2, 3, 4].into_iter().enumerate() {
        let chan = Arc::clone(&chan);
        let turn = Arc::clone(&turn);
        handles.push(thread::spawn(move || {
            while turn.load(Ordering::Acquire) != index {
                thread::yield_now();
            }
            chan.send(value);
            turn.fetch_add(1, Ordering::AcqRel);
        }));
    }
    for handle in handles {
        handle.join().expect("sender");
    }
    let received = (0..4).map(|_| chan.recv()).collect::<Vec<_>>();
    assert_eq!(received, vec![1, 2, 3, 4]);
}

#[test]
fn multiple_receivers_each_receive_once() {
    let chan = Arc::new(Chan::new(4));
    chan.send(10);
    chan.send(20);
    let handles = (0..2)
        .map(|_| {
            let chan = Arc::clone(&chan);
            thread::spawn(move || chan.recv())
        })
        .collect::<Vec<_>>();
    let mut received = handles
        .into_iter()
        .map(|handle| handle.join().expect("receiver"))
        .collect::<Vec<_>>();
    received.sort_unstable();
    assert_eq!(received, vec![10, 20]);
}
