use portable_pty::{MasterPty, PtySize};
use std::io::Write;
use std::sync::mpsc::{self as std_mpsc, Sender};
use std::thread;

enum PtyActorCommand {
    Write {
        data: Vec<u8>,
        ack: Option<std_mpsc::Sender<Result<(), String>>>,
    },
    Resize {
        size: PtySize,
        ack: std_mpsc::Sender<Result<(), String>>,
    },
    Shutdown {
        ack: std_mpsc::Sender<()>,
    },
}

#[derive(Clone)]
pub struct PtyActorHandle {
    sender: Sender<PtyActorCommand>,
}

pub fn spawn(
    master: Box<dyn MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
) -> PtyActorHandle {
    let (sender, receiver) = std_mpsc::channel::<PtyActorCommand>();

    thread::Builder::new()
        .name("pty-actor".to_string())
        .spawn(move || {
            while let Ok(cmd) = receiver.recv() {
                match cmd {
                    PtyActorCommand::Write { data, ack } => {
                        let res = writer
                            .write_all(&data)
                            .and_then(|_| writer.flush())
                            .map_err(|error| format!("ERR_PTY_WRITE|{error}"));
                        if let Some(ack_tx) = ack {
                            let _ = ack_tx.send(res);
                        }
                    }
                    PtyActorCommand::Resize { size, ack } => {
                        let res = master
                            .resize(size)
                            .map_err(|error| format!("ERR_PTY_RESIZE|{error}"));
                        let _ = ack.send(res);
                    }
                    PtyActorCommand::Shutdown { ack } => {
                        let _ = ack.send(());
                        break;
                    }
                }
            }
        })
        .expect("failed to spawn pty-actor thread");

    PtyActorHandle { sender }
}

impl PtyActorHandle {
    pub fn write(&self, data: Vec<u8>) -> Result<(), String> {
        let (ack_tx, ack_rx) = std_mpsc::channel();
        self.sender
            .send(PtyActorCommand::Write {
                data,
                ack: Some(ack_tx),
            })
            .map_err(|error| format!("ERR_PTY_ACTOR_CLOSED|{error}"))?;
        ack_rx
            .recv()
            .map_err(|error| format!("ERR_PTY_ACTOR_CLOSED|{error}"))?
    }

    pub fn write_async(&self, data: Vec<u8>) {
        let _ = self.sender.send(PtyActorCommand::Write { data, ack: None });
    }

    pub fn resize(&self, size: PtySize) -> Result<(), String> {
        let (ack_tx, ack_rx) = std_mpsc::channel();
        self.sender
            .send(PtyActorCommand::Resize { size, ack: ack_tx })
            .map_err(|error| format!("ERR_PTY_ACTOR_CLOSED|{error}"))?;
        ack_rx
            .recv()
            .map_err(|error| format!("ERR_PTY_ACTOR_CLOSED|{error}"))?
    }

    pub fn shutdown(&self) {
        let (ack_tx, ack_rx) = std_mpsc::channel();
        if self
            .sender
            .send(PtyActorCommand::Shutdown { ack: ack_tx })
            .is_ok()
        {
            let _ = ack_rx.recv();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use portable_pty::native_pty_system;
    use std::sync::{Arc, Mutex};

    struct MockWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
        flush_count: Arc<Mutex<usize>>,
    }

    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buffer.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            *self.flush_count.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn test_sync_write_and_flush() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty should succeed");

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let flush_count = Arc::new(Mutex::new(0));
        let mock_writer = MockWriter {
            buffer: buffer.clone(),
            flush_count: flush_count.clone(),
        };

        let actor = spawn(pair.master, Box::new(mock_writer));

        let res = actor.write(b"hello world\n".to_vec());
        assert!(res.is_ok());

        assert_eq!(*buffer.lock().unwrap(), b"hello world\n");
        assert_eq!(*flush_count.lock().unwrap(), 1);

        actor.shutdown();
    }

    #[test]
    fn test_async_write() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty should succeed");

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let flush_count = Arc::new(Mutex::new(0));
        let mock_writer = MockWriter {
            buffer: buffer.clone(),
            flush_count: flush_count.clone(),
        };

        let actor = spawn(pair.master, Box::new(mock_writer));

        actor.write_async(b"async chunk".to_vec());

        // A sync write acts as a barrier because commands are processed sequentially
        let res = actor.write(b"".to_vec());
        assert!(res.is_ok());

        assert_eq!(*buffer.lock().unwrap(), b"async chunk");
        assert_eq!(*flush_count.lock().unwrap(), 2);

        actor.shutdown();
    }

    #[test]
    fn test_resize() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty should succeed");

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let flush_count = Arc::new(Mutex::new(0));
        let mock_writer = MockWriter {
            buffer: buffer.clone(),
            flush_count: flush_count.clone(),
        };

        let actor = spawn(pair.master, Box::new(mock_writer));

        let res = actor.resize(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        });
        assert!(res.is_ok());

        actor.shutdown();
    }

    #[test]
    fn test_real_pty_read_write() {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty should succeed");

        let writer = pair
            .master
            .take_writer()
            .expect("take_writer should succeed");
        let actor = spawn(pair.master, writer);

        let res = actor.write(b"echo test\n".to_vec());
        assert!(res.is_ok());

        actor.shutdown();
    }
}
