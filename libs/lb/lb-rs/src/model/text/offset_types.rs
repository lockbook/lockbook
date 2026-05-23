use std::cmp::{Ordering, max, min};
use std::fmt::{Debug, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// A byte position in a buffer
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct Byte(pub usize);

/// A byte offset from a position in a buffer or a distance between two positions
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct Bytes(pub usize);

/// A character position in a buffer
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct Grapheme(pub usize);

/// A character offset from a position in a buffer or a distance between two positions
#[repr(transparent)]
#[derive(Default, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct Graphemes(pub usize);

// rel +/- rel = rel, doc +/- rel = doc, doc - doc = rel
impl Add<Bytes> for Bytes {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Bytes> for Bytes {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<Bytes> for Bytes {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign<Bytes> for Bytes {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}
impl Add<Bytes> for Byte {
    type Output = Self;

    fn add(self, rhs: Bytes) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Bytes> for Byte {
    type Output = Self;

    fn sub(self, rhs: Bytes) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<Bytes> for Byte {
    fn add_assign(&mut self, rhs: Bytes) {
        self.0 += rhs.0
    }
}

impl SubAssign<Bytes> for Byte {
    fn sub_assign(&mut self, rhs: Bytes) {
        self.0 -= rhs.0
    }
}

impl Sub<Byte> for Byte {
    type Output = Bytes;

    fn sub(self, rhs: Self) -> Self::Output {
        Bytes(self.0.saturating_sub(rhs.0))
    }
}

impl Add<Graphemes> for Graphemes {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Graphemes> for Graphemes {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<Graphemes> for Graphemes {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl SubAssign<Graphemes> for Graphemes {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}
impl Add<Graphemes> for Grapheme {
    type Output = Self;

    fn add(self, rhs: Graphemes) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Graphemes> for Grapheme {
    type Output = Self;

    fn sub(self, rhs: Graphemes) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl AddAssign<Graphemes> for Grapheme {
    fn add_assign(&mut self, rhs: Graphemes) {
        self.0 += rhs.0
    }
}

impl SubAssign<Graphemes> for Grapheme {
    fn sub_assign(&mut self, rhs: Graphemes) {
        self.0 -= rhs.0
    }
}

impl Sub<Grapheme> for Grapheme {
    type Output = Graphemes;

    fn sub(self, rhs: Self) -> Self::Output {
        Graphemes(self.0.saturating_sub(rhs.0))
    }
}

// all offset types impl From<usize>, PartialEq<usize>, PartialOrd<usize>, Add<usize>, Sub<usize>, AddAssign<usize>, SubAssign<usize>
impl From<usize> for Byte {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for Byte {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for Byte {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<Byte> for usize {
    fn eq(&self, other: &Byte) -> bool {
        self == &other.0
    }
}

impl PartialOrd<Byte> for usize {
    fn partial_cmp(&self, other: &Byte) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for Byte {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for Byte {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for Byte {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for Byte {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl From<usize> for Bytes {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for Bytes {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for Bytes {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<Bytes> for usize {
    fn eq(&self, other: &Bytes) -> bool {
        self == &other.0
    }
}

impl PartialOrd<Bytes> for usize {
    fn partial_cmp(&self, other: &Bytes) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for Bytes {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for Bytes {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for Bytes {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for Bytes {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl From<usize> for Grapheme {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for Grapheme {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for Grapheme {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<Grapheme> for usize {
    fn eq(&self, other: &Grapheme) -> bool {
        self == &other.0
    }
}

impl PartialOrd<Grapheme> for usize {
    fn partial_cmp(&self, other: &Grapheme) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for Grapheme {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for Grapheme {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for Grapheme {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for Grapheme {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl From<usize> for Graphemes {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PartialEq<usize> for Graphemes {
    fn eq(&self, other: &usize) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<usize> for Graphemes {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<Graphemes> for usize {
    fn eq(&self, other: &Graphemes) -> bool {
        self == &other.0
    }
}

impl PartialOrd<Graphemes> for usize {
    fn partial_cmp(&self, other: &Graphemes) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Add<usize> for Graphemes {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let sum = self.0 + rhs;
        Self(sum)
    }
}

impl Sub<usize> for Graphemes {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        let sum = self.0.saturating_sub(rhs);
        Self(sum)
    }
}

impl AddAssign<usize> for Graphemes {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign<usize> for Graphemes {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl Debug for Byte {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for Bytes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for Grapheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Debug for Graphemes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait RangeExt: Sized {
    type Element: Copy + Sub<Self::Element> + Ord;

    fn contains(&self, value: Self::Element, start_inclusive: bool, end_inclusive: bool) -> bool;
    fn intersects(
        &self, other: &(Self::Element, Self::Element), allow_empty_intersection: bool,
    ) -> bool;
    fn start(&self) -> Self::Element;
    fn end(&self) -> Self::Element;
    fn len(&self) -> <Self::Element as Sub>::Output;
    fn is_empty(&self) -> bool;

    fn contains_range(
        &self, value: &(Self::Element, Self::Element), start_inclusive: bool, end_inclusive: bool,
    ) -> bool {
        self.contains(value.0, start_inclusive, end_inclusive)
            && self.contains(value.1, start_inclusive, end_inclusive)
    }
    fn contains_inclusive(&self, value: Self::Element) -> bool {
        self.contains(value, true, true)
    }
    fn intersects_allow_empty(&self, other: &(Self::Element, Self::Element)) -> bool {
        self.intersects(other, true)
    }
    fn trim(&self, value: &(Self::Element, Self::Element)) -> (Self::Element, Self::Element) {
        (self.start().max(value.0).min(value.1), self.end().max(value.0).min(value.1))
    }
}

impl<T> RangeExt for (T, T)
where
    T: Ord + Sized + Copy + Sub<T>,
{
    type Element = T;

    /// returns whether the range includes the value
    fn contains(&self, value: T, start_inclusive: bool, end_inclusive: bool) -> bool {
        (self.start() < value || (start_inclusive && self.start() == value))
            && (value < self.end() || (end_inclusive && self.end() == value))
    }

    /// returns whether the range intersects another range
    fn intersects(&self, other: &(T, T), allow_empty_intersection: bool) -> bool {
        (self.start() < other.end() || (allow_empty_intersection && self.start() == other.end()))
            && (other.start() < self.end()
                || (allow_empty_intersection && other.start() == self.end()))
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

pub trait RangeIterExt {
    type Item;
    type Iter: DoubleEndedIterator<Item = Self::Item>;
    fn iter(self) -> Self::Iter;
}

impl<T: Copy + PartialOrd + AddAssign<usize> + SubAssign<usize>> RangeIterExt for (T, T) {
    type Item = T;
    type Iter = RangeIter<T>;
    fn iter(self) -> Self::Iter {
        RangeIter { start_inclusive: self.0, end_exclusive: self.1 }
    }
}

pub struct RangeIter<T> {
    start_inclusive: T,
    end_exclusive: T,
}

impl<T: Copy + PartialOrd + AddAssign<usize>> Iterator for RangeIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start_inclusive < self.end_exclusive {
            let result = self.start_inclusive;
            self.start_inclusive += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl<T: Copy + PartialOrd + AddAssign<usize> + SubAssign<usize>> DoubleEndedIterator
    for RangeIter<T>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start_inclusive < self.end_exclusive {
            self.end_exclusive -= 1;
            Some(self.end_exclusive)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use crate::model::text::offset_types::RangeExt as _;

    #[test]
    fn contains() {
        assert!(!(1, 3).contains(0, false, false));
        assert!(!(1, 3).contains(1, false, false));
        assert!((1, 3).contains(2, false, false));
        assert!(!(1, 3).contains(3, false, false));
        assert!(!(1, 3).contains(4, false, false));

        assert!(!(1, 3).contains(0, true, false));
        assert!((1, 3).contains(1, true, false));
        assert!((1, 3).contains(2, true, false));
        assert!(!(1, 3).contains(3, true, false));
        assert!(!(1, 3).contains(4, true, false));

        assert!(!(1, 3).contains(0, false, true));
        assert!(!(1, 3).contains(1, false, true));
        assert!((1, 3).contains(2, false, true));
        assert!((1, 3).contains(3, false, true));
        assert!(!(1, 3).contains(4, false, true));

        assert!(!(1, 3).contains(0, true, true));
        assert!((1, 3).contains(1, true, true));
        assert!((1, 3).contains(2, true, true));
        assert!((1, 3).contains(3, true, true));
        assert!(!(1, 3).contains(4, true, true));
    }

    #[test]
    fn contains_empty() {
        assert!(!(1, 1).contains(0, false, false));
        assert!(!(1, 1).contains(1, false, false));
        assert!(!(1, 1).contains(2, false, false));

        assert!(!(1, 1).contains(0, true, false));
        assert!(!(1, 1).contains(1, true, false));
        assert!(!(1, 1).contains(2, true, false));

        assert!(!(1, 1).contains(0, false, true));
        assert!(!(1, 1).contains(1, false, true));
        assert!(!(1, 1).contains(2, false, true));

        assert!(!(1, 1).contains(0, true, true));
        assert!((1, 1).contains(1, true, true));
        assert!(!(1, 1).contains(2, true, true));
    }

    #[test]
    fn intersects_allow_empty_contained() {
        assert!((1, 3).intersects(&(2, 2), false));
    }
}
