use usvg::Transform;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct DiffState {
    pub opacity_changed: bool,
    pub transformed: Option<Transform>,
    pub delete_changed: bool,
    pub data_changed: bool,
}

impl DiffState {
    /// is state dirty and require an i/o save
    pub fn is_dirty(&self) -> bool {
        self.data_changed
            || self.delete_changed
            || self.opacity_changed
            || self.transformed.is_some()
    }
}