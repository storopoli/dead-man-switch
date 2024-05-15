use crate::config::config_path;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct CliArgs {
    #[clap(short, long,value_parser, default_value_t = config_path().unwrap().to_str().unwrap().to_string())]
    pub config: String,
}
/// Parses arguments passed to the program.
pub fn check_args() -> Option<CliArgs> {
    let args = CliArgs::parse();
    Some(args)
}
