use std::path::PathBuf;

use recondo_domain::cmd;
// use recondo_domain::model;

fn main() {
    let upload_path = PathBuf::from("fubar.jpg");
    let media = cmd::create_media_from_file(upload_path);
    println!("Created media: {:?}", media);
}
