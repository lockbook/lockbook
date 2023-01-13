use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A character (not bytes) location in the whole document
#[repr(transparent)]
#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct DocCharOffset(pub usize);

/// A byte location inside the whole document
#[repr(transparent)]
#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct DocByteOffset(pub usize);

/// A character offset to a location within a LayoutJobInfo or GalleyInfo
/// This has not taken into account any head-modification
#[repr(transparent)]
#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct RelCharOffset(pub usize);

/// A character offset to a location within a Galley. This is after taking into account any
/// head modifications that may exist
#[repr(transparent)]
#[derive(Default, Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct GalleyOffset(pub usize);

impl From<usize> for DocByteOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for DocByteOffset {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for DocByteOffset {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Add<usize> for DocByteOffset {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for DocByteOffset {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0 - rhs;
        Self(sum)
    }
}

impl AddAssign<usize> for DocByteOffset {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for DocByteOffset {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl From<usize> for DocCharOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for DocCharOffset {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for DocCharOffset {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Add<usize> for DocCharOffset {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for DocCharOffset {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0 - rhs;
        Self(sum)
    }
}

impl AddAssign<usize> for DocCharOffset {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for DocCharOffset {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl Sub<DocCharOffset> for DocCharOffset {
    type Output = RelCharOffset;

    fn sub(self, rhs: Self) -> Self::Output {
        let rel = self.0 - rhs.0;
        RelCharOffset(rel)
    }
}

impl Sub<RelCharOffset> for DocCharOffset {
    type Output = Self;

    fn sub(self, rhs: RelCharOffset) -> Self::Output {
        let rel = self.0 - rhs.0;
        Self(rel)
    }
}

impl From<usize> for RelCharOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for RelCharOffset {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for RelCharOffset {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Sub<usize> for RelCharOffset {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let rel = self.0 - rhs;
        Self(rel)
    }
}

impl SubAssign<usize> for RelCharOffset {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}
