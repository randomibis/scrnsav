# AGENTS.md

Guidance for agents working on **scrnsav**, a Wayland screensaver for
GNOME/Mutter (Rust).

## What it is
- Two parts, xscreensaver-style: `scrnsav watch [secs]` (idle daemon, zbus →
  `org.gnome.Mutter.IdleMonitor`) spawns `scrnsav show` (fullscreen winit + wgpu
  renderer running a WGSL fragment shader).
- Visual saver **only**: GNOME Shell has no `ext-session-lock-v1` / layer-shell,
  so it cannot be or replace the lock screen. Don't attempt lock integration
  here — that would be a separate GNOME Shell extension project.

## Build / run / test
- `make build` (release), `make check` (fast type-check), `make test`
  (`cargo test` — **also validates every shader** in `shaders/` via naga).
- `make run` / `make run SHADER=shaders/ball.wgsl` to preview; `make run-all`
  cycles all shaders; `make watch IDLE=10` exercises the idle path.
- Run `make test` and `cargo fmt` before committing.
- GUI behaviour (looks right? dismiss works?) **can't be verified in CI** — ask
  the user to run `make run`; don't claim visual confirmation yourself.

## Pinned deps — read the real API, don't guess
- wgpu **30** and winit **0.30** are pinned; their APIs differ a lot from nearby
  versions and moved repeatedly (surface config, pipeline descriptors, error
  scopes, `get_current_texture`). When editing GPU/window code, check the actual
  types under `~/.cargo/registry/.../wgpu-30.0.1/` rather than guessing.

## Invariants to preserve
- **Uniform contract**: Rust `Uniforms` (src/render.rs) must byte-match the WGSL
  `struct Uniforms { time: f32, seed: f32, resolution: vec2<f32> }` (16 bytes).
  A test guards this — change a field and you update both sides + the test.
- **Shaders**: `shaders/*.wgsl`, need `vs_main`/`fs_main`, receive `time`,
  `seed` (per-monitor phase offset), `resolution`. Trails are analytical (no
  feedback buffer). Validate new ones with `make test`.
- **Dismiss split**: explicit `show` exits on **Escape only**; the daemon spawns
  `show --idle` for the classic any-input dismiss. Keep both.
- **Multi-monitor**: one window+surface per monitor, shared device/queue,
  per-monitor seed.
- **Shader compile errors** are caught via a wgpu validation error scope and
  surfaced as a clean error — don't reintroduce panics.

## Conventions
- Keep changes minimal. The maintainer prefers **happy-path tests only**; don't
  over-test. I/O-bound code (GPU/winit/zbus) is verified by manual runs.
- Match surrounding comment density and style.

## Committing
- Commits should be **GPG-signed**, but the key's passphrase isn't always cached
  in `gpg-agent`. Probe first (fails fast instead of prompting), then choose:

  ```sh
  KEY=$(git config user.signingkey)
  if echo probe | gpg --batch --pinentry-mode error ${KEY:+--local-user "$KEY"} \
       -o /dev/null -s 2>/dev/null; then
    git commit -S ...            # key is unlocked → sign
  else
    git commit --no-gpg-sign ... # locked: committing would prompt. Commit
                                 # unsigned and tell the user so they can
                                 # re-sign in a rebase.
  fi
  ```
