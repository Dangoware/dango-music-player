use file_format::{FileFormat, Kind};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::{error::Error, fs};
use walkdir::WalkDir;

use snap;

use super::library::{AlbumArt, Song, URI};
use unidecode::unidecode;

pub(super) fn normalize(input_string: &str) -> String {
    let mut normalized = unidecode(input_string);

    // Remove non alphanumeric characters
    normalized.retain(|c| c.is_alphanumeric());
    normalized = normalized.to_ascii_lowercase();

    normalized
}

pub(super) fn read_library(path: PathBuf) -> Result<Vec<Song>, Box<dyn Error>> {
    // Create a new snap reader over the database file
    let database = fs::File::open(path)?;
    let reader = BufReader::new(database);
    let mut d = snap::read::FrameDecoder::new(reader);

    // Decode the library from the serialized data into the vec
    let library: Vec<Song> = bincode::serde::decode_from_std_read(
        &mut d,
        bincode::config::standard()
            .with_little_endian()
            .with_variable_int_encoding(),
    )?;
    Ok(library)
}

pub(super) fn write_library(
    library: &Vec<Song>,
    path: PathBuf,
    take_backup: bool,
) -> Result<(), Box<dyn Error>> {
    // Create 2 new names for the file, a temporary one for writing out, and a backup
    let mut writer_name = path.clone();
    writer_name.set_extension("tmp");
    let mut backup_name = path.clone();
    backup_name.set_extension("bkp");

    // Create a new BufWriter on the file and make a snap frame encoer for it too
    let writer = BufWriter::new(fs::File::create(&writer_name)?);
    let mut e = snap::write::FrameEncoder::new(writer);

    // Write out the data using bincode
    bincode::serde::encode_into_std_write(
        library,
        &mut e,
        bincode::config::standard()
            .with_little_endian()
            .with_variable_int_encoding(),
    )?;

    if path.exists() && take_backup {
        fs::rename(&path, backup_name)?;
    }
    fs::rename(writer_name, &path)?;

    Ok(())
}

pub fn find_images(song_path: &Path) -> Result<Vec<AlbumArt>, Box<dyn Error>> {
    let mut images: Vec<AlbumArt> = Vec::new();

    let song_dir = song_path.parent().ok_or("")?;
    for target_file in WalkDir::new(song_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if target_file.depth() >= 3 {
            // Don't recurse very deep
            break;
        }

        let path = target_file.path();
        if !path.is_file() {
            continue;
        }

        let format = FileFormat::from_file(path)?.kind();
        if format != Kind::Image {
            break;
        }

        let image_uri = URI::Local(path.to_path_buf().canonicalize().unwrap());

        images.push(AlbumArt::External(image_uri));
    }

    Ok(images)
}
