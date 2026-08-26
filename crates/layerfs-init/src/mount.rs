use layerfs_core::{BootOptions, Checkpoint, LayerStack};

/// Resolves which layers a checkpoint needs, given the currently active
/// state. Pure decision logic; actual `mount(2)` calls happen in `assemble`.
pub fn resolve_stack(checkpoint: Checkpoint, _opts: &BootOptions) -> LayerStack {
    // TODO: populate from discovered backing-store layer paths once storage
    // discovery (locate LayerFS metadata) is implemented.
    let _ = checkpoint;
    LayerStack::new()
}

/// Assembles the final OverlayFS mount from a resolved layer stack and
/// switches into it. Not implemented outside a real initramfs environment.
pub fn assemble(_stack: &LayerStack) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "OverlayFS assembly not implemented yet",
    ))
}
