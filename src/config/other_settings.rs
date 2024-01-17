use std::{marker::PhantomData, fs::File, path::PathBuf};

use font::Font;

pub enum Setting {
    String {
        name: String,
        value: String
    },
    Int {
        name: String,
        value: i32
    },
    Bool {
        name: String,
        value: bool
    },

}

pub struct Form {

}

