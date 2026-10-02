mod idle;
mod render;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mode = std::env::args().nth(1).unwrap_or_else(|| "show".into());
    match mode.as_str() {
        "show" => render::run()?,
        "watch" => {
            // Idle timeout in seconds (default 5 min), passed to Mutter in ms.
            let secs: u64 = std::env::args()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(300);
            pollster::block_on(idle::run(secs * 1000))?;
        }
        "-h" | "--help" | "help" => print_help(),
        other => {
            eprintln!("scrnsav: unknown command '{other}'\n");
            print_help();
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "scrnsav — a Wayland/GNOME screensaver\n\
         \n\
         USAGE:\n\
         \u{20}   scrnsav show          Run the fullscreen saver now (exits on input)\n\
         \u{20}   scrnsav watch [secs]  Watch for idle and launch the saver (default 300s)\n"
    );
}
