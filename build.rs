use embuild::build::{CInclArgs, CfgArgs, LinkArgs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    CfgArgs::output_propagated("ESP_IDF")?;
    LinkArgs::output_propagated("ESP_IDF")?;
    let cfg = CfgArgs::try_from_env("ESP_IDF")?;
    if cfg.get("esp32c3").is_some() {
        build_init_usb()?
    }
    Ok(())
}

fn build_init_usb() -> Result<(), Box<dyn std::error::Error>> {
    let cincl = CInclArgs::try_from_env("ESP_IDF")?;
    let include_files = cincl
        .args
        .split_ascii_whitespace()
        .filter(|s| s.contains("-isystem"))
        .map(|s| s.replace("-isystem", "").replace('\"', ""));
    cc::Build::new()
        .file("src/c/initusb.c")
        .includes(include_files)
        .compile("initusb");
    Ok(())
}
