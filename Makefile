# scrnsav — build, run, and install targets.
# Everything goes through cargo; this is a convenience layer.

CARGO    ?= cargo
BIN       = scrnsav
RELEASE   = target/release/$(BIN)
DEBUG     = target/debug/$(BIN)

# Idle timeout (seconds) used by the `watch` / `run` targets.
IDLE     ?= 300

# Where the systemd user unit gets installed.
UNIT_DIR  = $(HOME)/.config/systemd/user
UNIT      = scrnsav.service

.DEFAULT_GOAL := build

## build: optimized release binary (what you install)
.PHONY: build
build:
	$(CARGO) build --release

## debug: unoptimized build with debug info
.PHONY: debug
debug:
	$(CARGO) build

## run: build release, then run the saver fullscreen now (any input exits)
.PHONY: run
run: build
	$(RELEASE) show

## watch: build release, then run the idle daemon (IDLE=<secs>, with logging)
.PHONY: watch
watch: build
	RUST_LOG=info $(RELEASE) watch $(IDLE)

## check: fast type-check without producing a binary
.PHONY: check
check:
	$(CARGO) check

## fmt: format the source
.PHONY: fmt
fmt:
	$(CARGO) fmt

## clippy: lint
.PHONY: clippy
clippy:
	$(CARGO) clippy --all-targets

## install: build + install and enable the systemd --user service
.PHONY: install
install: build
	mkdir -p $(UNIT_DIR)
	sed 's|%h/Code/randomibis/scrnsav/target/release/scrnsav|$(CURDIR)/$(RELEASE)|' \
		$(UNIT) > $(UNIT_DIR)/$(UNIT)
	systemctl --user daemon-reload
	systemctl --user enable --now $(UNIT)
	@echo "Installed. Tip: disable GNOME's own blank so scrnsav wins:"
	@echo "  gsettings set org.gnome.desktop.session idle-delay 0"

## uninstall: stop and remove the systemd --user service
.PHONY: uninstall
uninstall:
	-systemctl --user disable --now $(UNIT)
	-rm -f $(UNIT_DIR)/$(UNIT)
	systemctl --user daemon-reload

## clean: remove build artifacts
.PHONY: clean
clean:
	$(CARGO) clean

## help: list targets
.PHONY: help
help:
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## /  /'
