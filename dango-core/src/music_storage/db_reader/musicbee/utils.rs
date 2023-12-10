use leb128;

/// Gets a string from the MusicBee database format
///
/// The length of the string is defined by an LEB128 encoded value at the beginning, followed by the string of that length
pub fn get_string(iterator: &mut std::vec::IntoIter<u8>) -> String {
    let mut string_length = iterator.next().unwrap() as usize;
    if string_length == 0 {
        return String::new();
    }

    // Decode the LEB128 value
    let mut leb_bytes: Vec<u8> = vec![];
    loop {
        leb_bytes.push(string_length as u8);

        if string_length >> 7 != 1 {
            break;
        }
        string_length = iterator.next().unwrap() as usize;
    }
    string_length = leb128::read::unsigned(&mut leb_bytes.as_slice()).unwrap() as usize;

    let mut string_bytes = vec![];
    for _ in 0..string_length {
        string_bytes.push(iterator.next().unwrap());
    }
    String::from_utf8(string_bytes).unwrap()
}
