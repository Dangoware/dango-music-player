pub fn meta_offset(metadata: Vec<u8>, offset: usize) -> String {
    let mut result_vec = Vec::new();

    let mut i = offset;
    loop {
        if metadata[i] == 0x00 {
            break;
        }

        result_vec.push(metadata[i]);
        i += 1;
    }

    String::from_utf8_lossy(&result_vec).into()
}
