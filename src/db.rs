use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use serde::de::{self, Deserializer, Visitor};
use serde_json::Value as JsonValue;
use std::error::Error;
use std::fmt;

const SCHEMA: &'static str = "
create table if not exists events (id integer primary key, ts datetime, kind integer, data text);
create index if not exists events_ts on events (ts asc, kind);
";

/// Prepares the events database schema for the given SQLite connection.
pub fn initialize(conn: &Connection) -> Result<(), Box<Error>> {
    conn.execute(SCHEMA, &[])?;
    Ok(())
}

/// A single database event record. May be deserialized from a Janus JSON event blob.
#[derive(Debug, Deserialize)]
pub struct Event {
    /// Opaque Janus event data. May have been provided either by the core or a plugin.
    pub event: JsonValue,
    /// The Janus event category this event is in.
    #[serde(rename = "type")]
    pub kind: u32,
    /// The timestamp at which this event occurred.
    #[serde(deserialize_with = "deserialize_janus_timestamp")]
    pub timestamp: DateTime<Utc>,
}

fn deserialize_janus_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DateTimeFromJanusTimestampVisitor;
    impl<'de> Visitor<'de> for DateTimeFromJanusTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a UNIX timestamp")
        }

        fn visit_u64<E>(self, value: u64) -> Result<DateTime<Utc>, E>
        where
            E: de::Error,
        {
            // timestamps coming out of janus are in microseconds since epoch
            let seconds = value / 1000000;
            let nanos = 1000 * (value - (seconds * 1000000));
            Utc.timestamp_opt(seconds as i64, nanos as u32)
                .earliest()
                .ok_or_else(|| E::custom(format!("Value is not a legal timestamp: {}", value)))
        }
    }
    Ok(deserializer.deserialize_u64(DateTimeFromJanusTimestampVisitor)?)
}
