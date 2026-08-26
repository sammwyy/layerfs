/// Minimal deterministic boot logging. No journald dependency inside the
/// initramfs; everything goes to stdout/kmsg.
pub fn info(msg: &str) {
    println!("layerfs: {msg}");
}

pub fn debug(enabled: bool, msg: &str) {
    if enabled {
        println!("layerfs[debug]: {msg}");
    }
}

pub fn fatal(msg: &str) -> ! {
    eprintln!("layerfs: fatal: {msg}");
    std::process::exit(1);
}
