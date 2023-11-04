use std::io::{BufReader, BufWriter};
use std::{path::PathBuf, error::Error, fs};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;

use snap;

use unidecode::unidecode;
use crate::music_storage::music_db::Song;

pub fn normalize(input_string: &String) -> String {
    let mut normalized = unidecode(input_string);

    // Remove non alphanumeric characters
    normalized.retain(|c| c.is_alphabetic());
    normalized = normalized.to_ascii_lowercase();

    normalized
}

pub fn read_library(path: PathBuf) -> Result<Vec<Song>, Box<dyn Error>> {
    let database = fs::File::open(path)?;
    let reader = BufReader::new(database);
    //let mut d = ZlibDecoder::new(reader);

    let mut d = snap::read::FrameDecoder::new(reader);

    let library: Vec<Song> = bincode::serde::decode_from_std_read(&mut d, bincode::config::standard().with_little_endian().with_variable_int_encoding())?;
    Ok(library)
}

pub fn write_library(library: &Vec<Song>, path: PathBuf, take_backup: bool) -> Result<(), Box<dyn Error>> {
    let mut writer_name = path.clone();
    writer_name.set_extension("tmp");
    let mut backup_name = path.clone();
    backup_name.set_extension("bkp");

    let writer = BufWriter::new(fs::File::create(writer_name.to_path_buf())?);
    //let mut e = ZlibEncoder::new(writer, Compression::default());

    let mut e = snap::write::FrameEncoder::new(writer);

    bincode::serde::encode_into_std_write(&library, &mut e, bincode::config::standard().with_little_endian().with_variable_int_encoding())?;

    if path.exists() && take_backup {
        fs::rename(&path, backup_name)?;
    }
    fs::rename(writer_name, &path)?;

    Ok(())
}
