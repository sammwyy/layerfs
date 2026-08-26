use std::path::PathBuf;

/// Role a layer plays in the LayerFS composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerKind {
    Base,
    Update,
    UpdateHead,
    Override,
}

/// A single resolved, mountable layer on the backing store.
#[derive(Debug, Clone)]
pub struct Layer {
    pub kind: LayerKind,
    pub id: String,
    pub path: PathBuf,
    pub read_only: bool,
}

impl Layer {
    pub fn new(
        kind: LayerKind,
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        read_only: bool,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            path: path.into(),
            read_only,
        }
    }
}

/// Ordered OverlayFS composition, highest-priority layer first.
///
/// `layers[0]` is the writable upperdir when present; the rest are lowerdirs
/// in descending priority, matching section 4 of the design notes.
#[derive(Debug, Clone, Default)]
pub struct LayerStack {
    pub layers: Vec<Layer>,
}

impl LayerStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, layer: Layer) -> &mut Self {
        self.layers.push(layer);
        self
    }

    /// The upperdir, if this stack has a writable layer at the top.
    pub fn upper(&self) -> Option<&Layer> {
        self.layers.first().filter(|l| !l.read_only)
    }

    /// Lowerdirs in the order OverlayFS expects (highest priority first).
    pub fn lowers(&self) -> Vec<&Layer> {
        let skip = usize::from(self.upper().is_some());
        self.layers.iter().skip(skip).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_upper_from_lowers() {
        let mut stack = LayerStack::new();
        stack.push(Layer::new(
            LayerKind::Override,
            "override",
            "/override",
            false,
        ));
        stack.push(Layer::new(LayerKind::UpdateHead, "head-43", "/head", true));
        stack.push(Layer::new(LayerKind::Base, "base", "/base", true));

        assert_eq!(stack.upper().unwrap().id, "override");
        assert_eq!(stack.lowers().len(), 2);
    }

    #[test]
    fn read_only_stack_has_no_upper() {
        let mut stack = LayerStack::new();
        stack.push(Layer::new(LayerKind::Base, "base", "/base", true));
        assert!(stack.upper().is_none());
        assert_eq!(stack.lowers().len(), 1);
    }
}
