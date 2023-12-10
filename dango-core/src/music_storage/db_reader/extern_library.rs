use std::path::PathBuf;


pub trait ExternalLibrary {
    fn from_file(&mut self, file: &PathBuf) -> Self;
    fn write(&self) {
        unimplemented!();
    }
}