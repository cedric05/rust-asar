use clap::Parser;
use rust_asar::{AsarError, AsarFile};
mod cli;

fn main() -> Result<(), AsarError> {
    let args = cli::Args::parse();
    match args.command {
        cli::Commands::List { source } => {
            let asar_file = AsarFile::try_from(source)?;
            let list_files = asar_file.list_files()?;
            list_files.iter().for_each(|entry| {
                println!("entry {:?}", entry);
            });
        }
        cli::Commands::Extract { dest, source } => {
            let asar_file = AsarFile::try_from(source)?;
            asar_file.extract_all(dest.into())?;
        }
    }
    Ok(())
}
