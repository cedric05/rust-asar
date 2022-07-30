use clap::Parser;
use rust_asar::{AsarError, AsarFile, AsarFileEntry};
mod cli;

fn main() -> Result<(), AsarError> {
    let args = cli::Args::parse();
    match args.command {
        cli::Commands::List { source } => {
            let asar_file = AsarFile::try_from(source)?;
            let list_files = asar_file.list_files()?;
            list_files.iter().for_each(|entry| match entry {
                rust_asar::AsarEntry::File(AsarFileEntry {
                    filename,
                    offset: _,
                    size: _,
                }) => {
                    println!("{}", filename.to_string_lossy());
                }
                rust_asar::AsarEntry::Dir(_directory) => {
                    // println!("Dir: {}", directory);
                }
            });
        }
        cli::Commands::Extract { dest, source } => {
            let asar_file = AsarFile::try_from(source)?;
            asar_file.extract_all(dest.into())?;
        }
    }
    Ok(())
}
