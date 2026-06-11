use clap::Parser;

fn main() {
    let cli = contextcut::cli::Cli::parse();
    if let Err(err) = contextcut::run(&cli) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
