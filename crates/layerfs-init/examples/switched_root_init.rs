use std::io::Write;

use rustix::system::{RebootCommand, reboot};

fn main() {
    match std::fs::read_to_string("/handoff-marker") {
        Ok(marker) if marker.trim() == "override" => println!("QEMU-SWITCH-ROOT: PASS"),
        Ok(marker) => println!("QEMU-SWITCH-ROOT: FAIL: marker={}", marker.trim()),
        Err(error) => println!("QEMU-SWITCH-ROOT: FAIL: {error}"),
    }

    let _ = std::io::stdout().flush();
    let _ = reboot(RebootCommand::PowerOff);
}
