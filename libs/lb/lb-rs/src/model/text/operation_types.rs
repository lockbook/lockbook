use super::offset_types::DocCharOffset;

/// Buffer operation optimized for simplicity. Used in buffer's interface and internals to represent a building block
/// of text manipulation with support for undo/redo and collaborative editing.
#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Select((DocCharOffset, DocCharOffset)),
    Replace(Replace),
}

/// Represents the inverse of an operation in a particular application. Includes selection and optional replacement
/// because replacing text also affects the selection in ways that are not reversible based on the replacement alone.
#[derive(Clone, Debug)]
pub struct InverseOperation {
    pub select: (DocCharOffset, DocCharOffset),
    pub replace: Option<Replace>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Replace {
    pub range: (DocCharOffset, DocCharOffset),
    pub text: String,
}
