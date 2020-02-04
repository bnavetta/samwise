mod cli;
mod config;

mod device;
mod dhcp;

use std::fs;

use anyhow::{Context, Result};
use structopt::StructOpt;

use cli::Cli;
use config::Config;

fn main() -> Result<()> {
    let args = Cli::from_args();
    
    let config = {
        let config_str = fs::read_to_string(&args.config_path)
            .with_context(|| format!("Could not read configuration from {}", args.config_path.display()))?;
        Config::load(&config_str)?
    };

    println!("Config: {:?}", config);

    Ok(())
}
