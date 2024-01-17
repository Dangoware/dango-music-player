use std::{marker::PhantomData, fs::File, path::PathBuf};

use font::Font;

pub trait Setting {}

pub struct DropDown {
    name: String,
    //value: ???
}
impl Setting for DropDown {}

#[derive(Debug, Default)]
pub struct Slider {
    name: String,
    value: i32,
}
impl Setting for Slider {}

#[derive(Debug, Default)]
pub struct CheckBox {
    name: String,
    value: bool,
}
impl Setting for CheckBox {}

enum TextBoxSize {
    Small,
    Large,
}
#[derive(Debug, Default)]
pub struct TextBox<Size = TextBoxSize> {
    name: String,
    text: String,
    size: PhantomData<Size>
}
impl Setting for TextBox {}

#[derive(Debug, Default)]
pub struct SingleSelect {
    name: String,
    value: bool,
}
impl Setting for SingleSelect {}

#[derive(Debug, Default)]
pub struct MultiSelect {
    name: String,
    value: bool,
}
impl Setting for MultiSelect {}

#[derive(Debug, Default)]
pub struct ConfigCounter {
    name: String,
    value: i32,
}
impl Setting for ConfigCounter {}

#[derive(Debug, Default)]
pub struct ConfigFont {
    name: String,
    value: Font,
}
impl Setting for ConfigFont {}

#[derive(Debug, Default)]
pub struct ConfigFile {
    name: String,
    value: PathBuf,
}
impl Setting for ConfigFile {}

#[derive(Debug, Default)]
pub struct List<T: Setting> {
    items: Vec<T>
}

pub struct Form {

}

