//! A thread-local reference-counted slice type.

use core::prelude::*;

use core::{cmp, fmt, mem, ops};
use core::borrow::BorrowFrom;
use core::hash::{self, Hash};

use alloc::rc::{self, Rc, Weak};
use alloc::boxed::Box;


/// A reference-counted slice type.
///
/// This is exactly like `&[T]` except without lifetimes, so the
/// allocation only disappears once all `RcSlice`s have disappeared.
///
/// NB. this can lead to applications effectively leaking memory if a
/// short subslice of a long `RcSlice` is held.
///
/// # Examples
///
/// ```rust
/// use shared_slice::rc::RcSlice;
///
/// let x = RcSlice::new(Box::new(["foo", "bar", "baz"]));
/// println!("{:?}", x); // ["foo", "bar", "baz"]
/// println!("{:?}", x.slice(1, 3)); // ["bar", "baz"]
/// ```
///
/// Constructing with a dynamic number of elements:
///
/// ```rust
/// # #![allow(unstable)]
/// use shared_slice::rc::RcSlice;
///
/// let n = 5;
///
/// let v: Vec<u8> = (0u8..n).collect(); // 0, ..., 4
///
/// let x = RcSlice::new(v.into_boxed_slice());
/// assert_eq!(&*x, [0, 1, 2, 3, 4]);
/// ```
pub struct RcSlice<T> {
    data: *const [T],
    counts: Rc<()>,
}

/// A non-owning reference-counted slice type.
///
/// This is to `RcSlice` as `std::rc::Weak` is to `std::rc::Rc`, and
/// allows one to have cyclic references without stopping memory from
/// being deallocated.
pub struct WeakSlice<T> {
    data: *const [T],
    counts: Weak<()>,
}

impl<T> RcSlice<T> {
    /// Construct a new `RcSlice` containing the elements of `slice`.
    ///
    /// This reuses the allocation of `slice`.
    pub fn new(slice: Box<[T]>) -> RcSlice<T> {
        RcSlice {
            data: unsafe {mem::transmute(slice)},
            counts: Rc::new(())
        }
    }

    /// Downgrade self into a weak slice.
    pub fn downgrade(&self) -> WeakSlice<T> {
        WeakSlice {
            data: self.data,
            counts: self.counts.downgrade()
        }
    }

    /// Construct a new `RcSlice` that only points to elements at
    /// indices `lo` (inclusive) through `hi` (exclusive).
    ///
    /// This consumes `self` to avoid unnecessary reference-count
    /// modifications. Use `.clone()` if it is necessary to refer to
    /// `self` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if `lo > hi` or if either are strictly greater than
    /// `self.len()`.
    pub fn slice(mut self, lo: usize, hi: usize) -> RcSlice<T> {
        self.data = unsafe {&(&*self.data)[lo..hi]};
        self
    }
    /// Construct a new `RcSlice` that only points to elements at
    /// indices up to `hi` (exclusive).
    ///
    /// This consumes `self` to avoid unnecessary reference-count
    /// modifications. Use `.clone()` if it is necessary to refer to
    /// `self` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if `hi > self.len()`.
    pub fn slice_to(self, hi: usize) -> RcSlice<T> {
        self.slice(0, hi)
    }
    /// Construct a new `RcSlice` that only points to elements at
    /// indices starting at  `lo` (inclusive).
    ///
    /// This consumes `self` to avoid unnecessary reference-count
    /// modifications. Use `.clone()` if it is necessary to refer to
    /// `self` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if `lo > self.len()`.
    pub fn slice_from(self, lo: usize) -> RcSlice<T> {
        let hi = self.len();
        self.slice(lo, hi)
    }
}

impl<T> Clone for RcSlice<T> {
    fn clone(&self) -> RcSlice<T> {
        RcSlice {
            data: self.data,
            counts: self.counts.clone()
        }
    }
}

impl<T> BorrowFrom<RcSlice<T>> for [T] {
    fn borrow_from(owned: &RcSlice<T>) -> &[T] {
        &**owned
    }
}

impl<T> ops::Deref for RcSlice<T> {
    type Target = [T];
    fn deref<'a>(&'a self) -> &'a [T] {
        unsafe {&*self.data}
    }
}

impl<T: PartialEq> PartialEq for RcSlice<T> {
    fn eq(&self, other: &RcSlice<T>) -> bool { **self == **other }
    fn ne(&self, other: &RcSlice<T>) -> bool { **self != **other }
}
impl<T: Eq> Eq for RcSlice<T> {}

impl<T: PartialOrd> PartialOrd for RcSlice<T> {
    fn partial_cmp(&self, other: &RcSlice<T>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
    fn lt(&self, other: &RcSlice<T>) -> bool { **self < **other }
    fn le(&self, other: &RcSlice<T>) -> bool { **self <= **other }
    fn gt(&self, other: &RcSlice<T>) -> bool { **self > **other }
    fn ge(&self, other: &RcSlice<T>) -> bool { **self >= **other }
}
impl<T: Ord> Ord for RcSlice<T> {
    fn cmp(&self, other: &RcSlice<T>) -> cmp::Ordering { (**self).cmp(&**other) }
}

impl<S: hash::Hasher + hash::Writer, T: Hash<S>> Hash<S> for RcSlice<T> {
    fn hash(&self, state: &mut S) {
        (**self).hash(state)
    }
}

impl<T: fmt::Debug> fmt::Debug for RcSlice<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> WeakSlice<T> {
    /// Attempt to upgrade `self` to a strongly-counted `RcSlice`.
    ///
    /// Returns `None` if this is not possible (the data has already
    /// been freed).
    pub fn upgrade(&self) -> Option<RcSlice<T>> {
        self.counts.upgrade().map(|counts| {
            RcSlice {
                data: self.data,
                counts: counts
            }
        })
    }
}

// only RcSlice needs a destructor, since it entirely controls the
// actual allocated data; the deallocation of the counts (which is the
// only thing a WeakSlice needs to do if it is the very last pointer)
// is already handled by Rc<()>/Weak<()>.
#[unsafe_destructor]
impl<T> Drop for RcSlice<T> {
    fn drop(&mut self) {
        let strong = rc::strong_count(&self.counts);
        if strong == 1 {
            // last one, so let's clean up the stored data
            unsafe {
                let _: Box<[T]> = mem::transmute(self.data);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RcSlice, WeakSlice};
    use std::cell::Cell;
    use std::cmp::Ordering;
    #[test]
    fn clone() {
        let x = RcSlice::new(Box::new([Cell::new(false)]));
        let y = x.clone();

        assert_eq!(x[0].get(), false);
        assert_eq!(y[0].get(), false);

        x[0].set(true);
        assert_eq!(x[0].get(), true);
        assert_eq!(y[0].get(), true);
    }

    #[test]
    fn test_upgrade_downgrade() {
        let x = RcSlice::new(Box::new([1]));
        let y: WeakSlice<_> = x.downgrade();

        assert_eq!(y.upgrade(), Some(x.clone()));

        drop(x);

        assert!(y.upgrade().is_none())
    }

    #[test]
    fn test_total_cmp() {
        let x = RcSlice::new(Box::new([1, 2, 3]));
        let y = RcSlice::new(Box::new([1, 2, 3]));
        let z = RcSlice::new(Box::new([1, 2, 4]));
        assert_eq!(x, x);
        assert_eq!(x, y);
        assert!(x != z);
        assert!(y != z);

        assert!(x < z);
        assert!(x <= z);
        assert!(!(x > z));
        assert!(!(x >= z));

        assert!(!(z < x));
        assert!(!(z <= x));
        assert!(z > x);
        assert!(z >= x);

        assert_eq!(x.partial_cmp(&x), Some(Ordering::Equal));
        assert_eq!(x.partial_cmp(&y), Some(Ordering::Equal));
        assert_eq!(x.partial_cmp(&z), Some(Ordering::Less));
        assert_eq!(z.partial_cmp(&y), Some(Ordering::Greater));

        assert_eq!(x.cmp(&x), Ordering::Equal);
        assert_eq!(x.cmp(&y), Ordering::Equal);
        assert_eq!(x.cmp(&z), Ordering::Less);
        assert_eq!(z.cmp(&y), Ordering::Greater);
    }

    #[test]
    fn test_partial_cmp() {
        use std::f64;
        let x = RcSlice::new(Box::new([1.0, f64::NAN]));
        let y = RcSlice::new(Box::new([1.0, f64::NAN]));
        let z = RcSlice::new(Box::new([2.0, f64::NAN]));
        let w = RcSlice::new(Box::new([f64::NAN, 1.0]));
        assert!(!(x == y));
        assert!(x != y);

        assert!(!(x < y));
        assert!(!(x <= y));
        assert!(!(x > y));
        assert!(!(x >= y));

        assert!(x < z);
        assert!(x <= z);
        assert!(!(x > z));
        assert!(!(x >= z));

        assert!(!(z < w));
        assert!(!(z <= w));
        assert!(!(z > w));
        assert!(!(z >= w));

        assert_eq!(x.partial_cmp(&x), None);
        assert_eq!(x.partial_cmp(&y), None);
        assert_eq!(x.partial_cmp(&z), Some(Ordering::Less));
        assert_eq!(z.partial_cmp(&x), Some(Ordering::Greater));

        assert_eq!(x.partial_cmp(&w), None);
        assert_eq!(y.partial_cmp(&w), None);
        assert_eq!(z.partial_cmp(&w), None);
        assert_eq!(w.partial_cmp(&w), None);
    }

    #[test]
    fn test_show() {
        let x = RcSlice::new(Box::new([1, 2]));
        assert_eq!(format!("{:?}", x), "[1, 2]");

        let y: RcSlice<i32> = RcSlice::new(Box::new([]));
        assert_eq!(format!("{:?}", y), "[]");
    }

    #[test]
    fn test_slice() {
        let x = RcSlice::new(Box::new([1, 2, 3]));
        let real = [1, 2, 3];
        for i in range(0, 3 + 1) {
            for j in range(i, 3 + 1) {
                let slice: RcSlice<_> = x.clone().slice(i, j);
                assert_eq!(&*slice, &real[i..j]);
            }
            assert_eq!(&*x.clone().slice_to(i), &real[..i]);
            assert_eq!(&*x.clone().slice_from(i), &real[i..]);
        }
    }
}
