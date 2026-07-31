//! The incremental command-line entry point for ikashita.

const USAGE: &str = "ikashita 0.1.0\n\nUsage: ikashita [--help | --version]\n\nThe runtime and project commands are added incrementally behind this stable entry point.";

fn main() {
    let argument = std::env::args().nth(1);
    let exit_code = match argument.as_deref() {
        None | Some("--help") | Some("-h") => {
            println!("{USAGE}");
            0
        }
        Some("--version") | Some("-V") => {
            println!("ikashita {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some(unknown) => {
            eprintln!("unknown argument: {unknown}\n\n{USAGE}");
            2
        }
    };

    std::process::exit(exit_code);
}
