use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct CliArgs {
    #[clap(short, long)]
    pub config_path: Option<PathBuf>,
}

pub enum CliCommand {
    ConfigPath(PathBuf),
}
///This function checks for any commands passed to the program.
///Return the arguments passed to the program if any.
pub fn check_args() -> Option<CliArgs> {
    let args = CliArgs::parse();
    Some(args)
}
