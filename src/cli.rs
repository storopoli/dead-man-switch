use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct CliArgs {
    #[clap(short, long)]
    pub config: PathBuf,
}

pub enum CliCommand {
    ConfigPath(PathBuf),
}
/// Parses arguments passed to the program.
pub fn check_args() -> Option<CliArgs> {
    let args = CliArgs::parse();
    Some(args)
}
