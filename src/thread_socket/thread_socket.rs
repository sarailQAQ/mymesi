use std::sync::mpsc;

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
        ThreadSocket { sender, receiver }
    }

    pub fn receive(&self) -> T {
        self.receiver.recv().unwrap()
    }

    pub fn send(&self, data: T) {
        self.sender.send(data).unwrap();
    }
}

unsafe impl<T> Sync for ThreadSocket<T> {}

#[cfg(test)]
mod tests {
    use crate::thread_socket::thread_socket::ThreadSocket;
    use crate::thread_socket::thread_socket::*;
    use std::sync::mpsc::Sender;
    use std::time;
    use std::{future, thread};

    #[test]
    fn test_socket() {
        let (sa, sb): (ThreadSocket<String>, ThreadSocket<String>) = new_socket();

        let t1 = thread::spawn(move || {
            sa.send("msg1".to_string());
            sa.send("msg2".to_string());

            let msg = sa.receive();
            println!("a1: {:?}", msg);
            let msg = sa.receive();
            println!("a2: {:?}", msg);
        });

        let t2 = thread::spawn(move || {
            sb.send("msg3".to_string());
            sb.send("msg4".to_string());

            let msg = sb.receive();
            println!("b1: {:?}", msg);
            let msg = sb.receive();
            println!("b2: {:?}", msg);
        });

        t2.join().unwrap();
        t1.join().unwrap();
    }
}
