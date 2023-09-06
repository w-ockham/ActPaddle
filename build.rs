use std::process::Command;
// Necessary because of this issue: https://github.com/rust-lang/cargo/issues/9641
fn main() -> Result<(), Box<dyn std::error::Error>> {
    build_usb();
    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")?;
    Ok(())
}

fn build_usb() {
    let target = "./target/riscv32imc-esp-espidf/release/build/esp-idf-sys-3c0870045d375f7f/out/build/build.ninja";
    let cmd = Command::new("grep")
        .args(&["INCLUDES", target])
        .output()
        .expect("failed to start grep");
    let include_path = String::from_utf8_lossy(&cmd.stdout)
        .lines()
        .next()
        .unwrap()
        .replace("INCLUDES =   ", "")
        .replace("-I", "");
    let includes = include_path.split_whitespace();
    cc::Build::new()
        .file("src/c/initusb.c")
        .includes(includes)
        .compile("initusb");
}
