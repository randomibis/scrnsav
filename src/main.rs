mod idle;
mod render;

/// Parsed options shared by the subcommands.
struct Opts {
    /// Idle timeout in seconds (first bare number), used by `watch`.
    secs: Option<u64>,
    /// Path to a WGSL shader file (`--shader PATH`).
    shader: Option<String>,
    /// Internal: set by the idle daemon on the `show` it spawns, so the saver
    /// dismisses on any input rather than Escape-only.
    idle: bool,
}

fn parse_opts(args: &[String]) -> Opts {
    let mut secs = None;
    let mut shader = None;
    let mut idle = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--shader" | "-s" => {
                shader = args.get(i + 1).cloned();
                i += 2;
            }
            "--idle" => {
                idle = true;
                i += 1;
            }
            other => {
                if let Ok(n) = other.parse::<u64>() {
                    secs = Some(n);
                }
                i += 1;
            }
        }
    }
    Opts { secs, shader, idle }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("show");
    let rest = args.get(1..).unwrap_or(&[]);

    match mode {
        "show" => {
            let opts = parse_opts(rest);
            // Explicit run: Escape-only. Idle daemon passes --idle for any-input.
            let dismiss = if opts.idle {
                render::DismissMode::AnyInput
            } else {
                render::DismissMode::EscapeOnly
            };
            render::run(opts.shader, dismiss)?;
        }
        "watch" => {
            let opts = parse_opts(rest);
            let secs = opts.secs.unwrap_or(300);
            pollster::block_on(idle::run(secs * 1000, opts.shader))?;
        }
        "-h" | "--help" | "help" => print_help(),
        other => {
            eprintln!("scrnsav: unknown command '{other}'\n");
            print_help();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_secs_and_shader() {
        let args = [
            "60".to_string(),
            "--shader".to_string(),
            "x.wgsl".to_string(),
        ];
        let opts = parse_opts(&args);
        assert_eq!(opts.secs, Some(60));
        assert_eq!(opts.shader.as_deref(), Some("x.wgsl"));
    }
}

fn print_help() {
    println!(
        "scrnsav — a Wayland/GNOME screensaver\n\
         \n\
         USAGE:\n\
         \u{20}   scrnsav show  [--shader PATH]         Run the saver now (Esc to exit)\n\
         \u{20}   scrnsav watch [secs] [--shader PATH]  Watch for idle, then launch the saver\n\
         \n\
         Without --shader, a bundled default effect is used. Default idle is 300s.\n"
    );
}
