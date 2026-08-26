//! `rdinit=/init` that prints `/artifact-name` and powers off.

use rustix::system::{RebootCommand, reboot};

fn main() {
    match std::fs::read_to_string("/artifact-name") {
        Ok(name) => println!("ARTIFACT={}", name.trim()),
        Err(e) => println!("ARTIFACT-ERROR: {e}"),
    }

    use std::io::Write;
    let _ = std::io::stdout().flush();

    let _ = reboot(RebootCommand::PowerOff);
}
