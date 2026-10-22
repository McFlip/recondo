#[derive(Debug, Clone)]
pub struct Media {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
    pub view_count: u64,
}

#[derive(Debug, Clone)]
pub struct MediaOpts {
    pub mime_type: Option<String>,
    pub file_size: Option<u64>,
    pub view_count: Option<u64>,
}

impl Media {
    pub fn new(id: String, url: String, opts: Option<MediaOpts>) -> Self {
        Self {
            id,
            url,
            title: None,
            mime_type: opts.as_ref().and_then(|o| o.mime_type.clone()),
            file_size: opts.as_ref().and_then(|o| o.file_size),
            view_count: 0,
        }
    }
}
