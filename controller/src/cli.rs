use std::path::PathBuf;

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "samwise-controller", about = "Multiboot control server for Samwise")]
pub struct Cli {
    #[structopt(long = "--config")]
    #[structopt(parse(from_os_str))]
    pub config_path: PathBuf
}