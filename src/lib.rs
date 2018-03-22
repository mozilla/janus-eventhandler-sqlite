extern crate atom;
extern crate ini;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate janus_plugin as janus;
extern crate rusqlite;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

mod config;

use atom::AtomSetOnce;
use config::Config;
use janus::{LibraryMetadata, EventHandler, JanssonValue, JanssonDecodingFlags, RawJanssonValue};
use serde_json::Value as JsonValue;
use std::error::Error;
use std::ffi::CStr;
use std::os::raw::{c_int, c_char};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

#[derive(Debug)]
struct State {
    pub db: AtomSetOnce<Box<rusqlite::Connection>>,
    pub config: AtomSetOnce<Box<Config>>,
    pub event_channel: AtomSetOnce<Box<mpsc::SyncSender<RawEvent>>>,
}

lazy_static! {
    static ref STATE: State = State {
        db: AtomSetOnce::empty(),
        config: AtomSetOnce::empty(),
        event_channel: AtomSetOnce::empty(),
    };
}
/// A single event that happened. These will be queued up asynchronously and processed in order later.
#[derive(Debug)]
struct RawEvent {
    /// The event data.
    pub data: Option<JanssonValue>,
}

// courtesy of c_string crate, which also has some other stuff we aren't interested in
// taking in as a dependency here.
macro_rules! c_str {
    ($lit:expr) => {
        unsafe {
            CStr::from_ptr(concat!($lit, "\0").as_ptr() as *const $crate::c_char)
        }
    }
}

/// Inefficiently converts a serde JSON value to a Jansson JSON value.
fn from_serde_json(input: &JsonValue) -> JanssonValue {
    JanssonValue::from_str(&input.to_string(), JanssonDecodingFlags::empty()).unwrap()
}

fn get_config(config_root: *const c_char) -> Result<Config, Box<Error>> {
    unsafe {
        let config_path = Path::new(CStr::from_ptr(config_root).to_str()?);
        let config_file = config_path.join("janus.eventhandler.sqlite.cfg");
        Config::from_path(config_file)
    }
}

extern "C" fn init(config_path: *const c_char) -> c_int {
    let config = match get_config(config_path) {
        Ok(c) => c,
        Err(e) => {
            janus_warn!("Error loading configuration for event handler plugin: {}", e);
            Config::default()
        }
    };
    STATE.config.set_if_none(Box::new(config));
    let stored_config = STATE.config.get().unwrap();
    if stored_config.enabled {
        match rusqlite::Connection::open(&stored_config.db_path) {
            Ok(db) => {
                let (events_tx, events_rx) = mpsc::sync_channel(0);
                thread::spawn(move || {
                    janus_verb!("Event processing thread is alive.");
                    for ev in events_rx.iter() {
                        janus_verb!("Processing event: {:?}", ev);
                        handle_event_async(ev).err().map(|e| {
                            janus_err!("Error processing event: {}", e);
                        });
                    }
                });
                janus_info!("Recording events into SQLite database: {:?}", db);
                STATE.db.set_if_none(Box::new(db));
                STATE.event_channel.set_if_none(Box::new(events_tx));
                0
            }
            Err(e) => {
                janus_err!("Error opening SQLite event database: {}", e);
                -1
            }
        }
    } else {
        janus_warn!("Event handler plugin disabled.");
        -1
    }
}

extern "C" fn destroy() {
    janus_info!("Janus SQLite event recorder destroyed!");
}


extern "C" fn incoming_event(event: *mut RawJanssonValue) {
    if STATE.config.get().unwrap().enabled {
        janus_verb!("Queueing event.");
        let ev = RawEvent { data: unsafe { JanssonValue::new(event) }};
        STATE.event_channel.get().unwrap().send(ev).ok();
    }
}

fn handle_event_async(RawEvent { data }: RawEvent) -> Result<(), Box<Error>> {
    Ok(())
}

extern "C" fn handle_request(request: *mut RawJanssonValue) -> *mut RawJanssonValue {
    // we don't currently support runtime configuration of any kind
    from_serde_json(&json!({})).as_mut_ref()
}

const EVENTS_MASK: u32 = std::u32::MAX;

const EVH: EventHandler = build_eventhandler!(
    LibraryMetadata {
 api_version: 2,
        version: 1,
        name: c_str!("Janus SQLite event recorder"),
        package: c_str!("janus.eventhandler.sqlite"),
        version_str: c_str!(env!("CARGO_PKG_VERSION")),
        description: c_str!(env!("CARGO_PKG_DESCRIPTION")),
        author: c_str!(env!("CARGO_PKG_AUTHORS")),
    },
    EVENTS_MASK,
    init,
    destroy,
    incoming_event,
    handle_request
);

export_eventhandler!(&EVH);
