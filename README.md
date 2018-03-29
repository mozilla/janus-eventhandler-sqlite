# janus-eventhandler-sqlite

[![Build Status](https://travis-ci.org/mozilla/janus-eventhandler-sqlite.svg?branch=master)](https://travis-ci.org/mozilla/janus-eventhandler-sqlite)

A simple [Janus][] [event handler][Janus event handler] to record events in a SQLite database on disk.

## Configuration

This event handler will read janus.eventhandler.sqlite.cfg from the Janus config directory, if present. Like other Janus configuration, the config file should be in INI format. The following options are configurable in the `general` section of the config file:

- `enabled = yes|no`: Whether this event handler does any work at all. Default `yes`.
- `db_path = /path/to/sqlite/db`: The path to the SQLite DB in which events will be written. The database will be created and initialized if it's not already present. Defaults to `events.db`.
- `events = 65535`: A 32-bit integer bitmask defining which events will be logged. See the [Janus source][Janus event definitions] for valid event types. Defaults to all events.

## Dependencies

```
$ sudo apt install libjansson-dev libsqlite3-dev
```

## Building

```
$ cargo build [--release]
```

## Testing

```
$ cargo test
```

## Installing

Install the library output by the build process (e.g. ./target/release/libjanus_eventhandler_sqlite.so) into the Janus event handlers
directory (e.g. /usr/lib/janus/events). By default, event handlers may not be enabled in your Janus install; check your janus.cfg to make sure `broadcast=yes` is set in the `events` section. (If you are doing this for the first time, you might also want to double-check to make sure that there aren't other event handlers installed that you don't need.) Restart Janus to activate.

[Janus]: https://janus.conf.meetecho.com/
[Janus event handler]: https://janus.conf.meetecho.com/docs/group__eventhandlerapi.html
[Janus event definitions]: https://github.com/meetecho/janus-gateway/blob/master/events/eventhandler.h#L128
