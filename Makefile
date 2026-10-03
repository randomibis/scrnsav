# scrnsav — build, run, and install targets.
# Everything goes through cargo; this is a convenience layer.

CARGO    ?= cargo
BIN       = scrnsav
RELEASE   = target/release/$(BIN)
DEBUG     = target/debug/$(BIN)

# Idle timeout (seconds) used by the `watch` / `run` targets.
IDLE     ?= 300

# Optional WGSL shader file. Empty = bundled default effect.
#   make run SHADER=shaders/plasma.wgsl
SHADER   ?=
SHADER_ARG = $(if $(SHADER),--shader $(SHADER),)

# Extra flags for the `shots` target, e.g. make shots SHOT_ARGS="--size 1280x720"
SHOT_ARGS ?=

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
	$(RELEASE) show $(SHADER_ARG)

## run-all: Show each of the available shaders
.PHONY: run-all
run-all: build
	for s in shaders/*.wgsl; do \
	  $(RELEASE) show --shader $$s; \
	done

## shots: render a PNG of each shader to docs/shots/ (headless, no display)
.PHONY: shots
shots: build
	mkdir -p docs/shots
	for s in shaders/*.wgsl; do \
	  RUST_LOG=info $(RELEASE) shot --shader $$s \
	    --out docs/shots/$$(basename $$s .wgsl).png $(SHOT_ARGS); \
	done

## watch: build release, then run the idle daemon (IDLE=<secs>, with logging)
.PHONY: watch
watch: build
	RUST_LOG=info $(RELEASE) watch $(IDLE) $(SHADER_ARG)

## check: fast type-check without producing a binary
.PHONY: check
check:
	$(CARGO) check

## test: run tests (includes validating every shader in shaders/)
.PHONY: test
test:
	$(CARGO) test

## fmt: format the source
.PHONY: fmt
fmt:
	$(CARGO) fmt

## fmt-check: fail if the source isn't formatted (for CI)
.PHONY: fmt-check
fmt-check:
	$(CARGO) fmt --check

## clippy: lint
.PHONY: clippy
clippy:
	$(CARGO) clippy --all-targets

## install: build + install and enable the service (IDLE=<secs>, SHADER=<path>)
.PHONY: install
install: build
	mkdir -p $(UNIT_DIR)
	sed 's|^ExecStart=.*|ExecStart=$(CURDIR)/$(RELEASE) watch $(IDLE) $(SHADER_ARG)|' \
		$(UNIT) > $(UNIT_DIR)/$(UNIT)
	systemctl --user daemon-reload
	systemctl --user enable --now $(UNIT)
	@echo "Installed: watch $(IDLE)s. Tip: disable GNOME's own blank so scrnsav wins:"
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
