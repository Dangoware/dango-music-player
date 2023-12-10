use std::path::PathBuf;


pub trait ExternalLibrary {
    fn from_file(file: &PathBuf) -> Self;
    fn write(&self) {
        unimplemented!();
    }
}
