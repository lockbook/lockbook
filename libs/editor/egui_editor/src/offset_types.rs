use std::cmp::{max, min, Ordering};
use std::fmt::{Debug, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A byte position in a buffer
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct DocByteOffset(pub usize);

/// A byte offset from a position in a buffer or a distance between two positions
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct RelByteOffset(pub usize);

/// A character position in a buffer
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct DocCharOffset(pub usize);

/// A character offset from a position in a buffer or a distance between two positions
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct RelCharOffset(pub usize);

// rel +/- rel = rel, doc +/- rel = doc, doc - doc = rel
impl Add<RelByteOffset> for RelByteOffset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<RelByteOffset> for RelByteOffset {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<RelByteOffset> for RelByteOffset {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign<RelByteOffset> for RelByteOffset {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}
impl Add<RelByteOffset> for DocByteOffset {
    type Output = Self;

    fn add(self, rhs: RelByteOffset) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<RelByteOffset> for DocByteOffset {
    type Output = Self;

    fn sub(self, rhs: RelByteOffset) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<RelByteOffset> for DocByteOffset {
    fn add_assign(&mut self, rhs: RelByteOffset) {
        self.0 += rhs.0
    }
}

impl SubAssign<RelByteOffset> for DocByteOffset {
    fn sub_assign(&mut self, rhs: RelByteOffset) {
        self.0 -= rhs.0
    }
}

impl Sub<DocByteOffset> for DocByteOffset {
    type Output = RelByteOffset;

    fn sub(self, rhs: Self) -> Self::Output {
        RelByteOffset(self.0.saturating_sub(rhs.0))
    }
}

impl Add<RelCharOffset> for RelCharOffset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<RelCharOffset> for RelCharOffset {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<RelCharOffset> for RelCharOffset {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign<RelCharOffset> for RelCharOffset {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}
impl Add<RelCharOffset> for DocCharOffset {
    type Output = Self;

    fn add(self, rhs: RelCharOffset) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<RelCharOffset> for DocCharOffset {
    type Output = Self;

    fn sub(self, rhs: RelCharOffset) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<RelCharOffset> for DocCharOffset {
    fn add_assign(&mut self, rhs: RelCharOffset) {
        self.0 += rhs.0
    }
}

impl SubAssign<RelCharOffset> for DocCharOffset {
    fn sub_assign(&mut self, rhs: RelCharOffset) {
        self.0 -= rhs.0
    }
}

impl Sub<DocCharOffset> for DocCharOffset {
    type Output = RelCharOffset;

    fn sub(self, rhs: Self) -> Self::Output {
        RelCharOffset(self.0.saturating_sub(rhs.0))
    }
}

// all offset types impl From<usize>, PartialEq<usize>, PartialOrd<usize>, Add<usize>, Sub<usize>, AddAssign<usize>, SubAssign<usize>
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

impl PartialEq<DocByteOffset> for usize {
    fn eq(&self, other: &DocByteOffset) -> bool {
        self == &other.0
    }
}

impl PartialOrd<DocByteOffset> for usize {
    fn partial_cmp(&self, other: &DocByteOffset) -> Option<Ordering> {
        self.partial_cmp(&other.0)
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
        let sum = self.0.saturating_sub(rhs);
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

impl From<usize> for RelByteOffset {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for RelByteOffset {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for RelByteOffset {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<RelByteOffset> for usize {
    fn eq(&self, other: &RelByteOffset) -> bool {
        self == &other.0
    }
}

impl PartialOrd<RelByteOffset> for usize {
    fn partial_cmp(&self, other: &RelByteOffset) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for RelByteOffset {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for RelByteOffset {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for RelByteOffset {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for RelByteOffset {
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

impl PartialEq<DocCharOffset> for usize {
    fn eq(&self, other: &DocCharOffset) -> bool {
        self == &other.0
    }
}

impl PartialOrd<DocCharOffset> for usize {
    fn partial_cmp(&self, other: &DocCharOffset) -> Option<Ordering> {
        self.partial_cmp(&other.0)
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
        let sum = self.0.saturating_sub(rhs);
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

impl PartialEq<RelCharOffset> for usize {
    fn eq(&self, other: &RelCharOffset) -> bool {
        self == &other.0
    }
}

impl PartialOrd<RelCharOffset> for usize {
    fn partial_cmp(&self, other: &RelCharOffset) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for RelCharOffset {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for RelCharOffset {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for RelCharOffset {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for RelCharOffset {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl Debug for DocByteOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for RelByteOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for DocCharOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for RelCharOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait RangeExt<Element: Sub<Element>>: Ord + Sized {
    fn contains(&self, value: Element) -> bool;
    fn start(&self) -> Element;
    fn end(&self) -> Element;
    fn len(&self) -> <Element as Sub>::Output;
    fn is_empty(&self) -> bool;
}

impl<T> RangeExt<T> for (T, T)
where
    T: Ord + Sized + Copy + Sub<T>,
{
    /// returns whether the range includes the value, treating the (start, end) bounds as (inclusive, exclusive)
    fn contains(&self, value: T) -> bool {
        self.start() <= value && value < self.end()
    }

    fn start(&self) -> T {
        *min(&self.0, &self.1)
    }

    fn end(&self) -> T {
        *max(&self.0, &self.1)
    }

    fn len(&self) -> <T as Sub>::Output {
        self.end() - self.start()
    }

    fn is_empty(&self) -> bool {
        self.0 == self.1
    }
}

pub trait ToRangeExt: Sized {
    fn to_range(self) -> (Self, Self);
}

impl<T> ToRangeExt for T
where
    T: Copy,
{
    fn to_range(self) -> (Self, Self) {
        (self, self)
    }
}

pub trait IntoRangeExt<I> {
    fn into_range(self) -> (I, I);
}

impl<T, I> IntoRangeExt<I> for T
where
    T: Copy + Into<I>,
{
    fn into_range(self) -> (I, I) {
        (self.into(), self.into())
    }
}
