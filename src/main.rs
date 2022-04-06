use std::path::PathBuf;
use structopt::StructOpt;

mod client;
mod engine;
mod tx;

#[derive(Debug, StructOpt)]
/// Parses CSV input file with txs, processes them and outputs the state of clients as CSV
struct Opt {
    #[structopt(parse(from_os_str))]
    input_csv: PathBuf,
}

fn main() -> anyhow::Result<()> {
    engine::Engine::default().run(Opt::from_args().input_csv)
}
