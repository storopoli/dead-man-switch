use clap::Parser;

use crate::config::config_path;

/// CLI Arguments.
#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct DmsArgs {
    /// Path to TOML [`Config`](crate::config::Config).
    #[clap(short, long, default_value_t = config_path().expect("Failed to get config path").to_str().unwrap().to_string())]
    pub config: String,
}

/// Parses arguments passed to the Deadman's Switch.
pub fn parse_args() -> DmsArgs {
    DmsArgs::parse()
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_clap() {
        DmsArgs::command().debug_assert()
    }
}
