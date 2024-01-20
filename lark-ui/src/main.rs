use clap::Parser;

mod cli;
mod tui;

fn main() {
    let opts = cli::Opts::parse();

    tui::App::new(opts).run().expect("Failed to initialize TUI");
}
