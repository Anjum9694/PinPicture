// quick decode probe: cargo run --release --bin probe -- <image path>
fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: probe <image>");
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{path}: READ FAIL {e}");
            return;
        }
    };
    let rd = match image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{path}: FORMAT FAIL {e}");
            return;
        }
    };
    eprintln!("{path}: format = {:?}", rd.format());
    match rd.decode() {
        Ok(im) => {
            use image::GenericImageView;
            let (w, h) = (im.width(), im.height());
            let c = im.get_pixel(w / 2, h / 2);
            let t = im.get_pixel(50, 50);
            eprintln!(
                "{path}: decoded {}x{}  center={:?}  (50,50)={:?}",
                w, h, c, t
            );
        }
        Err(e) => eprintln!("{path}: DECODE FAIL {e}"),
    }
}
