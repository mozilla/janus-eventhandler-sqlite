PREFIX = /opt/janus/lib/janus/events
TARGET = target/release/libjanus_eventhandler_sqlite.so

install:
	cargo build --release
	cargo test --release
	mkdir -p $(DESTDIR)$(PREFIX)
	cp $(TARGET) $(DESTDIR)$(PREFIX)

.PHONY: install
