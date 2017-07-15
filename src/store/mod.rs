//! Zone persistence
//!
//! The Store "process" handles Zone persistence.
//!
//! Zones can load data or request to save data. When requesting to save data, `Store` will notify
//! the Zone when it is not busy, at which point the Zone can send its latest copy of its data.

pub mod fs;
pub mod null;

use std::error::Error;
use std::fmt;
use std::sync::mpsc::{channel, Receiver, Sender};

use bincode;

use path::Path;
use zone::{ZoneData, ZoneHandle};

/// A handle to the Store process. This is the shareable public interface.
#[derive(Clone)]
pub struct StoreHandle {
    tx: Sender<StoreCall>
}

/// Channel (both ends) to talk to Store, `rx` needed to spawn Store.
pub struct StoreChannel {
    rx: Receiver<StoreCall>,
    tx: Sender<StoreCall>
}

/// Used for dispatching calls via message passing.
pub enum StoreCall {
    List(Sender<Path>),
    Load(ZoneHandle, Path),
    LoadData(Path, Sender<Option<ZoneData>>),
    RequestWrite(ZoneHandle),
    Write(ZoneHandle, Path, Vec<u8>)
}

/// Storage error that includes generic Error-implementing errors
#[derive(Debug)]
pub enum StoreError {
    ReadError(Box<Error>),
    OtherError(Box<Error>),
    WriteError(Box<Error>)
}

impl StoreChannel {
    pub fn new() -> StoreChannel {
        let (tx, rx) = channel();

        StoreChannel { rx: rx, tx: tx }
    }

    pub fn handle(&self) -> StoreHandle {
        StoreHandle { tx: self.tx.clone() }
    }
}

impl StoreHandle {
    /// Gets a list of Zone Paths stored locally
    pub fn each_zone<F>(&self, mut f: F) where F: FnMut(Path) {
        let (tx, rx) = channel();

        self.tx.send(StoreCall::List(tx)).unwrap();

        for p in rx.iter() {
            f(p)
        }
    }

    /// Reads data for a given zone path and sends data back directly to the `Zone` asynchronously.
    pub fn load(&self, zone: &ZoneHandle, path: &Path) {
        self.tx.send(StoreCall::Load(zone.clone(), path.clone())).unwrap();
    }

    /// Reads data for a given zone path and returns it.
    pub fn load_data(&self, path: Path) -> Option<ZoneData> {
        let (tx, rx) = channel();

        self.tx.send(StoreCall::LoadData(path, tx)).unwrap();

        rx.recv().unwrap()
    }

    /// Ask for non-busy write notification.
    pub fn request_write(&self, zone: &ZoneHandle) {
        self.tx.send(StoreCall::RequestWrite(zone.clone())).unwrap();
    }

    /// Saves data for a zone and notifies zone directly via its handle.
    pub fn write(&self, zone: &ZoneHandle, path: &Path, data: &ZoneData) {
        // Optimization: seralize to send over channel instead of cloning ZoneData
        let limit = bincode::Infinite;
        let serialized = bincode::serialize(&data, limit).unwrap();

        self.tx.send(StoreCall::Write(zone.clone(), path.clone(), serialized)).unwrap();
    }

    /// Creates a noop StoreHandle for testing
    #[cfg(test)]
    pub fn test_handle() -> StoreHandle {
        use std::sync::mpsc::channel;

        StoreHandle {
            tx: channel().0
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            StoreError::ReadError(ref err) => write!(f, "Read error: {}", err.description()),
            StoreError::OtherError(ref err) => write!(f, "Other error: {}", err.description()),
            StoreError::WriteError(ref err) => write!(f, "Write error: {}", err.description())
        }
    }
}

impl Error for StoreError {
    fn description(&self) -> &str {
        match *self {
            StoreError::ReadError(ref err) => err.description(),
            StoreError::OtherError(ref err) => err.description(),
            StoreError::WriteError(ref err) => err.description()
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            StoreError::ReadError(ref err) => Some(&**err),
            StoreError::OtherError(ref err) => Some(&**err),
            StoreError::WriteError(ref err) => Some(&**err)
        }
    }
}
