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

#[cfg(test)]
mod test {
    use crate::config::load_or_initialize_config;
    use std::path::PathBuf;

    use super::*;
    use clap::CommandFactory;
    #[test]
    fn test_clap() {
        DmsArgs::command().debug_assert()
    }
    #[test]
    fn test_provided_config() {
        let args = DmsArgs {
            config: "./config.example.toml".to_string(),
        };
        let config = load_or_initialize_config(PathBuf::from(args.config)).unwrap();
        assert_eq!(config.timer_dead_man, 604800);
    }
}
