BIN_DIR ?= $(HOME)/.local/bin

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	install -m 755 target/release/rust-dock $(BIN_DIR)/rust-dock

uninstall:
	rm -f $(BIN_DIR)/rust-dock

clean:
	cargo clean
