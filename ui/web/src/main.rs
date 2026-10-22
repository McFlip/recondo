use recondo_domain::cmd;
// use recondo_domain::model;

fn main() {
    let media = cmd::create_media_from_file();
    println!("Created media: {:?}", media);
}
