use std::{
    fs::{create_dir_all, File},
    io::{Read, Seek, SeekFrom},
    num::ParseIntError,
    os::unix::prelude::AsRawFd,
    path::PathBuf,
};

use libc::*;
use nom::{number::streaming::le_u32, sequence::tuple};
use serde_json::Value;

#[allow(dead_code)]
pub struct AsarFile {
    fd: File,
    header_size: u64,
    json_size: u64,
    header: Value,
}

#[derive(Debug)]
pub enum AsarError {
    IO(std::io::Error),
    Nom,
    FormatError,
    Unknown,
    InvalidDirectory,
    JsonHeaderError,
}

impl From<std::io::Error> for AsarError {
    fn from(error: std::io::Error) -> Self {
        AsarError::IO(error)
    }
}

impl From<serde_json::Error> for AsarError {
    fn from(_error: serde_json::Error) -> Self {
        AsarError::JsonHeaderError
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for AsarError {
    fn from(_error: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        AsarError::Nom
    }
}

impl From<ParseIntError> for AsarError {
    fn from(_error: ParseIntError) -> Self {
        AsarError::JsonHeaderError
    }
}

impl TryFrom<&str> for AsarFile {
    type Error = AsarError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let file = File::open(value)?;
        AsarFile::new(file)
    }
}

impl TryFrom<String> for AsarFile {
    type Error = AsarError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let file = File::open(value)?;
        AsarFile::new(file)
    }
}

#[derive(Debug)]
pub struct AsarFileEntry {
    pub filename: PathBuf,
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug)]
pub enum AsarEntry {
    File(AsarFileEntry),
    Dir(String),
}

pub fn parse_header(input: &[u8]) -> Result<(u32, u32), nom::Err<nom::error::Error<&[u8]>>> {
    let (_, (_, header_size, _, json_size)) = tuple((le_u32, le_u32, le_u32, le_u32))(input)?;
    Ok((header_size, json_size))
}

impl AsarFile {
    fn new(mut file: File) -> Result<AsarFile, AsarError> {
        let mut b = [0u8; 16];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut b)?;
        let (header_size, json_size) = parse_header(&b)?;
        let mut b = vec![0; json_size as usize];
        file.read_exact(&mut b)?;
        let value = serde_json::from_slice(&b)?;
        Ok(AsarFile {
            fd: file,
            header_size: header_size as u64,
            json_size: json_size as u64,
            header: value,
        })
    }

    pub fn read_header(
        value: &Value,
        result: &mut Vec<AsarEntry>,
        root_entry: PathBuf,
    ) -> Result<(), AsarError> {
        let object = value.as_object().ok_or(AsarError::JsonHeaderError)?;
        for (key, value) in object.iter() {
            let sub = value.as_object().ok_or(AsarError::JsonHeaderError)?;
            if sub.contains_key("files") {
                result.push(AsarEntry::Dir(key.clone()));
                let sub = sub.get("files").ok_or(AsarError::JsonHeaderError)?;
                AsarFile::read_header(sub, result, root_entry.join(key))?;
            } else {
                let size = sub
                    .get("size")
                    .ok_or(AsarError::JsonHeaderError)?
                    .as_u64()
                    .ok_or(AsarError::JsonHeaderError)?;
                let offset = sub
                    .get("offset")
                    .ok_or(AsarError::JsonHeaderError)?
                    .as_str()
                    .ok_or(AsarError::JsonHeaderError)?;
                let offset: u64 = offset.parse()?;
                result.push(AsarEntry::File(AsarFileEntry {
                    filename: root_entry.join(key),
                    offset,
                    size,
                }));
            }
        }
        Ok(())
    }

    pub fn list_files(&self) -> Result<Vec<AsarEntry>, AsarError> {
        let mut vec = vec![];
        let object = self.header.as_object().ok_or(AsarError::JsonHeaderError)?;
        let value = object.get("files").ok_or(AsarError::JsonHeaderError)?;
        AsarFile::read_header(value, &mut vec, "".into())?;
        Ok(vec)
    }

    // avoid using this
    // loads everything into memory
    // you want to read chunk
    pub fn read_content(
        &mut self,
        entry: &AsarEntry,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match entry {
            AsarEntry::File(fileentry) => {
                let AsarFileEntry {
                    filename: _filename,
                    offset,
                    size,
                } = fileentry;
                self.fd
                    .seek(SeekFrom::Start(self.header_size + 8 + offset))?;
                let mut bytes = vec![0u8; (*size).try_into().unwrap()];
                self.fd.read_exact(&mut bytes)?;
                Ok(bytes)
            }
            AsarEntry::Dir(_directory) => Ok(vec![]),
        }
    }

    // avoid using this
    pub fn read_string(&mut self, entry: &AsarEntry) -> Result<String, Box<dyn std::error::Error>> {
        match self.read_content(entry) {
            Ok(content) => Ok(String::from_utf8_lossy(&content).to_string()),
            Err(err) => Err(err),
        }
    }

    pub fn extract_all(self, directory: PathBuf) -> std::result::Result<(), AsarError> {
        if directory.exists() {
            if !directory.is_dir() {
                return Err(AsarError::InvalidDirectory);
            }
        } else {
            create_dir_all(&directory)?;
        }
        let result = self
            .list_files()?
            .iter()
            .map(|entry| {
                match entry {
                    AsarEntry::File(file_entry) => {
                        let AsarFileEntry {
                            filename,
                            offset,
                            size,
                        } = file_entry;
                        let filename = directory.join(filename);
                        let file_out = File::options().write(true).create(true).open(&filename)?;
                        let mut offset = (self.header_size + 8 + offset) as i64;
                        unsafe {
                            let res = sendfile(
                                file_out.as_raw_fd(),
                                self.fd.as_raw_fd(),
                                &mut offset,
                                *size as usize,
                            );
                            if res == -1 {
                                println!(
                                    "ran into error whlie writing {}",
                                    filename.to_string_lossy()
                                );
                                return Err(AsarError::Unknown);
                            } else {
                                println!(
                                    "created file {} and wrote {} bytes",
                                    filename.to_string_lossy(),
                                    size
                                );
                            };
                        }
                    }
                    AsarEntry::Dir(directory_name) => {
                        create_dir_all(directory.join(directory_name)).unwrap()
                    }
                };
                Ok(())
            })
            .find(|res| res.is_err());
        if let Some(error) = result {
            error
        } else {
            Ok(())
        }
    }
}
