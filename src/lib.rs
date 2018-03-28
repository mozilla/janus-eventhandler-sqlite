extern crate atom;
extern crate chrono;
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
mod db;

use atom::AtomSetOnce;
use config::Config;
use janus::{EventHandler, JanssonDecodingFlags, JanssonEncodingFlags, JanssonValue, LibraryMetadata, RawJanssonValue};
use rusqlite::Connection;
use serde_json::Value as JsonValue;
use std::error::Error;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::mpsc;
use std::thread;

// courtesy of c_string crate, which also has some other stuff we aren't interested in
// taking in as a dependency here.
macro_rules! c_str {
    ($lit: expr) => {
        unsafe { CStr::from_ptr(concat!($lit, "\0").as_ptr() as *const $crate::c_char) }
    };
}

/// Inefficiently converts a serde JSON value to a Jansson JSON value.
fn from_serde_json(input: &JsonValue) -> JanssonValue {
    JanssonValue::from_str(&input.to_string(), JanssonDecodingFlags::empty()).unwrap()
}

#[derive(Debug)]
struct State {
    pub config: AtomSetOnce<Box<Config>>,
    pub event_channel: AtomSetOnce<Box<mpsc::SyncSender<RawEvent>>>,
}

lazy_static! {
    static ref STATE: State = State {
        config: AtomSetOnce::empty(),
        event_channel: AtomSetOnce::empty(),
    };
}

/// A single event that happened. These will be queued up asynchronously and processed in order later.
#[derive(Debug)]
struct RawEvent {
    /// JSON information associated with the event. Probably the best reference to the contents of Janus events is the
    /// sample event handler code in the Janus codebase, which you can find here:
    /// https://github.com/meetecho/janus-gateway/blob/master/events/janus_sampleevh.c#L473
    pub json: Option<JanssonValue>,
}

fn get_config(config_root: *const c_char) -> Result<Config, Box<Error>> {
    let config_path = unsafe { Path::new(CStr::from_ptr(config_root).to_str()?) };
    let config_file = config_path.join("janus.eventhandler.sqlite.cfg");
    Config::from_path(config_file)
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
        let (events_tx, events_rx) = mpsc::sync_channel(0);
        STATE.event_channel.set_if_none(Box::new(events_tx));
        thread::spawn(move || {
            if let Err(e) = handle_events(stored_config, events_rx) {
                janus_err!("Error running event processor: {}", e);
            }
        });
        0
    } else {
        janus_warn!("Event handler plugin disabled.");
        -1
    }
}

extern "C" fn destroy() {
    janus_info!("Janus SQLite event recorder destroyed!");
}

extern "C" fn handle_request(request: *mut RawJanssonValue) -> *mut RawJanssonValue {
    // we don't currently support runtime reconfiguration or queries of any kind, although we could
    from_serde_json(&json!({})).as_mut_ref()
}

extern "C" fn incoming_event(event: *mut RawJanssonValue) {
    if STATE.config.get().unwrap().enabled {
        let ev = RawEvent {
            json: unsafe { JanssonValue::from_and_incref(event) },
        };
        STATE.event_channel.get().unwrap().send(ev).ok();
    }
}

fn parse_event(RawEvent { json }: RawEvent) -> Result<db::Event, Box<Error>> {
    match json {
        None => Err(From::from("Events should not be null.")),
        Some(data) => Ok(serde_json::from_str(data.to_libcstring(JanssonEncodingFlags::empty()).to_str()?)?),
    }
}

fn handle_events(config: &Config, events_rx: mpsc::Receiver<RawEvent>) -> Result<(), Box<Error>> {
    let conn = Connection::open(&config.db_path)?;
    db::initialize(&conn)?;
    janus_info!("Recording events into SQLite database: {:?}", conn);
    let mut insert_event = conn.prepare("insert into events (ts, kind, data) values (:ts, :kind, :data)")?;
    for ev in events_rx.iter() {
        match parse_event(ev) {
            Err(e) => {
                janus_err!("Error parsing event: {}", e);
            }
            Ok(parsed) => match insert_event.execute(&[&parsed.timestamp, &parsed.kind, &parsed.event]) {
                Ok(1) => {
                    janus_verb!("Inserted event: {:?}", parsed);
                }
                Ok(_) => {
                    janus_err!("Insert of event failed: {:?}", parsed);
                }
                Err(e) => {
                    janus_err!("Insert of event failed ({}): {:?}", e, parsed);
                }
            },
        }
    }
    Ok(())
}

const EVENTS_MASK: u32 = std::u32::MAX; // todo: would be nice if this was configurable

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
