use layerfs_core::{Checkpoint, Layer, LayerKind, LayerStack};
use layerfs_storage::DiscoveredStore;

pub use layerfs_storage::overlay::{assemble, mount_data};

/// Builds the layer stack for a checkpoint from discovered store paths.
/// Pure decision logic: a layer is included only if the checkpoint calls
/// for it and the backing directory actually exists.
pub fn resolve_stack(
    checkpoint: Checkpoint,
    head: bool,
    discovered: &DiscoveredStore,
) -> LayerStack {
    let mut stack = LayerStack::new();

    if checkpoint.includes_override()
        && let Some(path) = &discovered.r#override
    {
        stack.push(Layer::new(
            LayerKind::Override,
            "override",
            path.clone(),
            false,
        ));
    }

    if checkpoint.includes_update() {
        if head && let Some(path) = &discovered.update_head {
            stack.push(Layer::new(
                LayerKind::UpdateHead,
                "update-head",
                path.clone(),
                true,
            ));
        }
        if let Some(path) = &discovered.update {
            stack.push(Layer::new(LayerKind::Update, "update", path.clone(), true));
        }
    }

    stack.push(Layer::new(
        LayerKind::Base,
        "base",
        discovered.base.clone(),
        true,
    ));

    stack
}

#[cfg(test)]
mod tests {
    use super::*;
    use layerfs_storage::DiscoveredStore;
    use std::path::PathBuf;

    fn discovered() -> DiscoveredStore {
        DiscoveredStore {
            base: PathBuf::from("/store/base"),
            update: Some(PathBuf::from("/store/update")),
            update_head: Some(PathBuf::from("/store/update-head")),
            r#override: Some(PathBuf::from("/store/override")),
            data: None,
            work: PathBuf::from("/store/work"),
        }
    }

    #[test]
    fn base_checkpoint_is_base_only() {
        let stack = resolve_stack(Checkpoint::Base, true, &discovered());
        assert_eq!(stack.layers.len(), 1);
        assert_eq!(stack.layers[0].kind, LayerKind::Base);
    }

    #[test]
    fn normal_checkpoint_includes_full_stack() {
        let stack = resolve_stack(Checkpoint::Normal, true, &discovered());
        let kinds: Vec<_> = stack.layers.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LayerKind::Override,
                LayerKind::UpdateHead,
                LayerKind::Update,
                LayerKind::Base
            ]
        );
    }

    #[test]
    fn head_off_drops_update_head_only() {
        let stack = resolve_stack(Checkpoint::Safe, false, &discovered());
        let kinds: Vec<_> = stack.layers.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, [LayerKind::Update, LayerKind::Base]);
    }

    #[test]
    fn missing_layers_are_skipped_not_faked() {
        let mut d = discovered();
        d.update_head = None;
        let stack = resolve_stack(Checkpoint::Normal, true, &d);
        let kinds: Vec<_> = stack.layers.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [LayerKind::Override, LayerKind::Update, LayerKind::Base]
        );
    }
}
