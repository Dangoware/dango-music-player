use ciborium::{from_reader, into_writer};
use deunicode::deunicode_with_tofu;
use file_format::{FileFormat, Kind};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::library::{AlbumArt, URI};

#[cfg(target_family = "windows")]
use std::os::windows::fs::MetadataExt;

pub(super) fn normalize(input_string: &str) -> String {
    // Normalize the string to latin characters... this needs a lot of work
    let mut normalized = deunicode_with_tofu(input_string, " ");

    // Remove non alphanumeric characters
    normalized.retain(|c| c.is_alphanumeric());
    normalized = normalized.to_ascii_lowercase();

    normalized
}

/// Write any data structure which implements [serde::Serialize]
/// out to a [cbor] encoded file compressed using [ciborium]
pub(super) fn write_file<
    T: serde::Serialize,
    U: std::convert::AsRef<Path> + std::convert::AsRef<std::ffi::OsStr> + Clone,
>(
    library: T,
    path: U,
) -> Result<(), Box<dyn Error>> {
    // Create a temporary name for writing out
    let mut writer_name = PathBuf::from(&path);
    writer_name.set_extension("tmp");

    // Create a new BufWriter on the file and a snap frame encoder
    let writer = BufWriter::new(File::create(&writer_name)?);
    //let mut e = snap::write::FrameEncoder::new(writer);

    // Write out the data
    into_writer(&library, writer)?;
    fs::rename(writer_name, &path)?;

    Ok(())
}

/// Read a file serialized out with [write_file] and turn it into
/// the desired structure
pub(super) fn read_file<T: for<'de> serde::Deserialize<'de>>(
    path: PathBuf,
) -> Result<T, Box<dyn Error>> {
    // Create a new snap reader over the file
    let file_reader = BufReader::new(OpenOptions::new().read(true).open(path)?);
    //let mut d = snap::read::FrameDecoder::new(file_reader);

    // Decode the library from the serialized data into the vec
    let library: T = from_reader(file_reader)?;

    Ok(library)
}

pub fn find_images(song_path: &Path) -> Result<Vec<AlbumArt>, Box<dyn Error>> {
    let mut images: Vec<AlbumArt> = Vec::new();

    let song_dir = song_path.parent().ok_or("")?;
    for target_file in WalkDir::new(song_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() < 3)
    // Don't recurse very deep
    {
        let path = target_file.path();
        if !path.is_file() || !path.exists() {
            continue;
        }

        let format = FileFormat::from_file(path)?.kind();
        if format != Kind::Image {
            continue;
        }

        #[cfg(target_family = "windows")]
        if (4 & path.metadata().unwrap().file_attributes()) == 4 {
            continue;
        }

        let image_uri = URI::Local(path.to_path_buf().canonicalize()?);

        images.push(AlbumArt::External(image_uri));
    }

    Ok(images)
}
