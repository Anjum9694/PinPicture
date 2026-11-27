fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/ico_white.ico");
    res.set("ProductName", "PinPicture");
    res.set("FileDescription", "Pin any picture to the desktop");
    res.set("LegalCopyright", "MIT");
    if let Err(e) = res.compile() {
        println!("cargo:warning=winresource failed: {e}");
    }
}
