//! Cross-crate smoke test: boot option parsing feeds checkpoint resolution
//! the way layerfs-init consumes it.

use layerfs_core::{BootOptions, Checkpoint};

#[test]
fn safe_with_head_off_matches_previous_update_grub_entry() {
    let opts = BootOptions::parse("layerfs.checkpoint=safe layerfs.head=off").unwrap();
    assert_eq!(opts.checkpoint, Checkpoint::Safe);
    assert!(!opts.head);
    assert!(opts.checkpoint.includes_data());
    assert!(!opts.checkpoint.includes_override());
}
