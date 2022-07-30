use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    List {
        /// source asar file name
        source: String,
    },
    Extract {
        /// source asar file name
        source: String,
        /// dest directory
        dest: String,
    },
}
