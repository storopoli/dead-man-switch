use clap::Parser;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    #[arg(short, long)]
    ConfigPath: Option<PathBuff>,
}

#[derive(Error, Debug)]
pub enum CliErrors {
    /// IO operations on cli module
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}
pub fn check_args() -> Result<(), CliErrors> {
    let args = CliArgs::parse();
    if !args.ConfigPath.is_none() {
        
    }
    return Ok();
}
