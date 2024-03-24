use std::any::Any;
use std::fs::{File, self};
use std::io::{BufReader, BufWriter};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::error::Error;

use walkdir::WalkDir;
use file_format::{FileFormat, Kind};
use snap;
use deunicode::deunicode_with_tofu;

use super::library::{AlbumArt, URI};

pub(super) fn normalize(input_string: &str) -> String {
    // Normalize the string to latin characters... this needs a lot of work
    let mut normalized = deunicode_with_tofu(input_string, " ");

    // Remove non alphanumeric characters
    normalized.retain(|c| c.is_alphanumeric());
    normalized = normalized.to_ascii_lowercase();

    normalized
}

/// Write any data structure which implements [serde::Serialize]
/// out to a [bincode] encoded file compressed using [snap]
pub(super) fn write_file<T: serde::Serialize>(
    library: T,
    path: PathBuf,
) -> Result<(), Box<dyn Error>> {
    // Create a temporary name for writing out
    let mut writer_name = path.clone();
    writer_name.set_extension("tmp");

    // Create a new BufWriter on the file and a snap frame encoder
    let writer = BufWriter::new(File::create(&writer_name)?);
    let mut e = snap::write::FrameEncoder::new(writer);

    // Write out the data
    bincode::serde::encode_into_std_write(
        library,
        &mut e,
        bincode::config::standard()
            .with_little_endian()
            .with_variable_int_encoding(),
    )?;
    fs::rename(writer_name, &path)?;

    Ok(())
}

/// Read a file serialized out with [write_file] and turn it into
/// the desired structure
pub(super) fn read_file<T: for<'de> serde::Deserialize<'de>>(path: PathBuf) -> Result<T, Box<dyn Error>> {
    // Create a new snap reader over the file
    let file_reader = BufReader::new(File::open(path)?);
    let mut d = snap::read::FrameDecoder::new(file_reader);

    // Decode the library from the serialized data into the vec
    let library: T = bincode::serde::decode_from_std_read(
        &mut d,
        bincode::config::standard()
            .with_little_endian()
            .with_variable_int_encoding(),
    )?;

    Ok(library)
}

pub fn find_images(song_path: &Path) -> Result<Vec<AlbumArt>, Box<dyn Error>> {
    let mut images: Vec<AlbumArt> = Vec::new();

    let song_dir = song_path.parent().ok_or("")?;
    for target_file in WalkDir::new(song_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() < 3) // Don't recurse very deep
    {
        // println!("{:?}", target_file);
        let path = target_file.path();
        if !path.is_file() || !path.exists() {
            continue;
        }

        let format = FileFormat::from_file(path)?.kind();
        if format != Kind::Image {
            continue;
        }

        #[cfg(target_family = "windows")]
        if (4 & path.metadata().unwrap().file_attributes()) == 4  {
            continue;
        }

        let image_uri = URI::Local(path.to_path_buf().canonicalize()?);

        images.push(AlbumArt::External(image_uri));
    }

    Ok(images)
}
