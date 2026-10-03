//! Idle watcher for GNOME/Mutter.
//!
//! GNOME Shell doesn't implement `ext-session-lock-v1` or layer-shell, so we
//! can't be a "real" locker. What we *can* do is ask Mutter to tell us when the
//! session goes idle, and launch the saver window ourselves.
//!
//! D-Bus interface: org.gnome.Mutter.IdleMonitor on the session bus.

use std::process::Child;

use anyhow::Context;
use futures_util::StreamExt;

#[zbus::proxy(
    interface = "org.gnome.Mutter.IdleMonitor",
    default_service = "org.gnome.Mutter.IdleMonitor",
    default_path = "/org/gnome/Mutter/IdleMonitor/Core"
)]
trait IdleMonitor {
    /// Fires once when the session has been idle for `interval` milliseconds.
    /// Re-arms automatically for the next idle period.
    fn add_idle_watch(&self, interval: u64) -> zbus::Result<u32>;

    /// Fires once the next time the user becomes active. One-shot.
    fn add_user_active_watch(&self) -> zbus::Result<u32>;

    fn remove_watch(&self, id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn watch_fired(&self, id: u32) -> zbus::Result<()>;
}

pub async fn run(timeout_ms: u64, shader: Option<String>) -> anyhow::Result<()> {
    let conn = zbus::Connection::session()
        .await
        .context("connecting to the session bus")?;
    let proxy = IdleMonitorProxy::new(&conn)
        .await
        .context("creating Mutter IdleMonitor proxy")?;

    let idle_id = proxy
        .add_idle_watch(timeout_ms)
        .await
        .context("registering idle watch")?;
    log::info!("watching for {timeout_ms}ms of idle (watch id {idle_id})");

    let mut fired = proxy.receive_watch_fired().await?;
    let mut child: Option<Child> = None;
    let mut active_id: Option<u32> = None;

    while let Some(signal) = fired.next().await {
        let id = signal.args()?.id;

        if id == idle_id {
            // Became idle. Launch the saver if it isn't already up.
            let running = child
                .as_mut()
                .map(|c| matches!(c.try_wait(), Ok(None)))
                .unwrap_or(false);
            if running {
                continue;
            }
            match spawn_show(shader.as_deref()) {
                Ok(c) => {
                    log::info!("idle: launched saver (pid {})", c.id());
                    child = Some(c);
                    // Arm a one-shot watch so we can tear down when the user returns.
                    active_id = proxy.add_user_active_watch().await.ok();
                }
                Err(e) => log::error!("failed to launch saver: {e:#}"),
            }
        } else if Some(id) == active_id {
            // User came back. The saver normally exits on its own input event,
            // but kill it defensively in case the input went elsewhere.
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            active_id = None;
            log::info!("user active: saver dismissed");
        }
    }

    Ok(())
}

fn spawn_show(shader: Option<&str>) -> anyhow::Result<Child> {
    let exe = std::env::current_exe().context("resolving own executable path")?;
    let mut cmd = std::process::Command::new(exe);
    // --idle keeps the classic any-input dismiss for the idle-triggered saver;
    // an explicit `scrnsav show` omits it and exits on Escape only.
    cmd.arg("show").arg("--idle");
    if let Some(path) = shader {
        cmd.arg("--shader").arg(path);
    }
    cmd.spawn().context("spawning saver process")
}
