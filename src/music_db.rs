mod music_db {
    use uuid::Uuid;
    use std::path::Path;
    use time::Date;
    use file_format::FileFormat;
    use std::time::Duration;
    
    struct Song {
        uuid: Uuid,
        path: Box<Path>,
        title: String,
        album: String,
        tracknum: usize,
        artist: String,
        plays: usize,
        favorited: bool,
        date: Date,
        format: FileFormat,
        duration: Duration,
        genre: String,
        rating: i8,
    }
    
    pub fn test(){
        
    }
    
}