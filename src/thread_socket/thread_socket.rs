
use std::{
    sync::mpsc,
};

pub fn new_socket<T>() -> (ThreadSocket<T>, ThreadSocket<T>) {
    let (sender_a, receiver_b) = mpsc::channel();
    let (sender_b, receiver_a) = mpsc::channel();

    let socket_a: ThreadSocket<T> = ThreadSocket::new(sender_a, receiver_a);
    let socket_b: ThreadSocket<T> = ThreadSocket::new(sender_b, receiver_b);

    (socket_a, socket_b)
}

pub struct ThreadSocket<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,
}

impl<T> ThreadSocket<T> {
    fn new(sender: mpsc::Sender<T>, receiver: mpsc::Receiver<T>) -> ThreadSocket<T> {
        ThreadSocket {sender, receiver}
    }

    fn receive(&self) -> T {
        self.receiver.recv()?
    }

    fn send(&self, data: T) {
        self.sender.send(data)?;
    }
}