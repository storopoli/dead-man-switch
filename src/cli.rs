use crate::config::config_path;
use clap::Parser;
fn default_config_path() -> String {
    config_path().unwrap().to_str().unwrap().to_string()
}

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct DmsArgs {
    #[clap(short, long,value_parser, default_value_t = default_config_path())]
    pub config: String,
}
/// Parses arguments passed to the program.
pub fn check_args() -> DmsArgs {
    DmsArgs::parse()
}
