//! A thread-safe reference-counted slice type.

use core::prelude::*;

use core::{fmt, mem};
use core::borrow::BorrowFrom;
use core::hash::{mod, Hash};

use alloc::arc::{mod, Arc, Weak};
use alloc::boxed::Box;


/// A reference-counted slice type.
///
/// This is exactly like `&[T]` except without lifetimes, so the
/// allocation only disappears once all `ArcSlice`s have disappeared.
///
/// NB. this can lead to applications effectively leaking memory if a
/// short subslice of a long `ArcSlice` is held.
///
/// # Examples
///
/// ```rust
/// use shared_slice::arc::ArcSlice;
///
/// let x = ArcSlice::new(box ["foo", "bar", "baz"]);
/// println!("{}", x); // [foo, bar, baz]
/// println!("{}", x.slice(1, 3)); // [bar, baz]
/// ```
///
/// Constructing with a dynamic number of elements:
///
/// ```rust
/// use shared_slice::arc::ArcSlice;
///
/// let n = 5;
///
/// let v: Vec<u8> = range(0u8, n).collect(); // 0, ..., 4
///
/// let x = ArcSlice::new(v.into_boxed_slice());
/// assert_eq!(&*x, [0, 1, 2, 3, 4]);
/// ```
pub struct ArcSlice<T> {
    data: *const [T],
    counts: Arc<()>,
}

unsafe impl<T: Send + Sync> Send for ArcSlice<T> {}
unsafe impl<T: Send + Sync> Sync for ArcSlice<T> {}

/// A non-owning reference-counted slice type.
///
/// This is to `ArcSlice` as `std::sync::Weak` is to `std::sync::Arc`, and
/// allows one to have cyclic references without stopping memory from
/// being deallocated.
pub struct WeakSlice<T> {
    data: *const [T],
    counts: Weak<()>,
}
unsafe impl<T: Send + Sync> Send for WeakSlice<T> {}
unsafe impl<T: Send + Sync> Sync for WeakSlice<T> {}

impl<T> ArcSlice<T> {
    /// Construct a new `ArcSlice` containing the elements of `slice`.
    ///
    /// This reuses the allocation of `slice`.
    pub fn new(slice: Box<[T]>) -> ArcSlice<T> {
        ArcSlice {
            data: unsafe {mem::transmute(slice)},
            counts: Arc::new(())
        }
    }

    /// Downgrade self into a weak slice.
    pub fn downgrade(&self) -> WeakSlice<T> {
        WeakSlice {
            data: self.data,
            counts: self.counts.downgrade()
        }
    }

    /// Construct a new `ArcSlice` that only points to elements at
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
    pub fn slice(mut self, lo: uint, hi: uint) -> ArcSlice<T> {
        self.data = unsafe {(&*self.data).slice(lo, hi)};
        self
    }
    /// Construct a new `ArcSlice` that only points to elements at
    /// indices up to `hi` (exclusive).
    ///
    /// This consumes `self` to avoid unnecessary reference-count
    /// modifications. Use `.clone()` if it is necessary to refer to
    /// `self` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if `hi > self.len()`.
    pub fn slice_to(self, hi: uint) -> ArcSlice<T> {
        self.slice(0, hi)
    }
    /// Construct a new `ArcSlice` that only points to elements at
    /// indices starting at  `lo` (inclusive).
    ///
    /// This consumes `self` to avoid unnecessary reference-count
    /// modifications. Use `.clone()` if it is necessary to refer to
    /// `self` after calling this.
    ///
    /// # Panics
    ///
    /// Panics if `lo > self.len()`.
    pub fn slice_from(self, lo: uint) -> ArcSlice<T> {
        let hi = self.len();
        self.slice(lo, hi)
    }
}

impl<T> Clone for ArcSlice<T> {
    fn clone(&self) -> ArcSlice<T> {
        ArcSlice {
            data: self.data,
            counts: self.counts.clone()
        }
    }
}

impl<T> BorrowFrom<ArcSlice<T>> for [T] {
    fn borrow_from(owned: &ArcSlice<T>) -> &[T] {
        &**owned
    }
}

impl<T> Deref<[T]> for ArcSlice<T> {
    fn deref<'a>(&'a self) -> &'a [T] {
        unsafe {&*self.data}
    }
}

impl<T: PartialEq> PartialEq for ArcSlice<T> {
    fn eq(&self, other: &ArcSlice<T>) -> bool { **self == **other }
    fn ne(&self, other: &ArcSlice<T>) -> bool { **self != **other }
}
impl<T: Eq> Eq for ArcSlice<T> {}

impl<T: PartialOrd> PartialOrd for ArcSlice<T> {
    fn partial_cmp(&self, other: &ArcSlice<T>) -> Option<Ordering> { (**self).partial_cmp(&**other) }
    fn lt(&self, other: &ArcSlice<T>) -> bool { **self < **other }
    fn le(&self, other: &ArcSlice<T>) -> bool { **self <= **other }
    fn gt(&self, other: &ArcSlice<T>) -> bool { **self > **other }
    fn ge(&self, other: &ArcSlice<T>) -> bool { **self >= **other }
}
impl<T: Ord> Ord for ArcSlice<T> {
    fn cmp(&self, other: &ArcSlice<T>) -> Ordering { (**self).cmp(&**other) }
}

impl<S: hash::Writer, T: Hash<S>> Hash<S> for ArcSlice<T> {
    fn hash(&self, state: &mut S) {
        (**self).hash(state)
    }
}

impl<T: fmt::Show> fmt::Show for ArcSlice<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> WeakSlice<T> {
    /// Attempt to upgrade `self` to a strongly-counted `ArcSlice`.
    ///
    /// Returns `None` if this is not possible (the data has already
    /// been freed).
    pub fn upgrade(&self) -> Option<ArcSlice<T>> {
        self.counts.upgrade().map(|counts| {
            ArcSlice {
                data: self.data,
                counts: counts
            }
        })
    }
}

// only ArcSlice needs a destructor, since it entirely controls the
// actual allocated data; the deallocation of the counts (which is the
// only thing a WeakSlice needs to do if it is the very last pointer)
// is already handled by Arc<()>/Weak<()>.
#[unsafe_destructor]
impl<T> Drop for ArcSlice<T> {
    fn drop(&mut self) {
        let strong = arc::strong_count(&self.counts);
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
    use super::{ArcSlice, WeakSlice};
    use std::cell::Cell;
    #[test]
    fn clone() {
        let x = ArcSlice::new(box [Cell::new(false)]);
        let y = x.clone();

        assert_eq!(x[0].get(), false);
        assert_eq!(y[0].get(), false);

        x[0].set(true);
        assert_eq!(x[0].get(), true);
        assert_eq!(y[0].get(), true);
    }

    #[test]
    fn test_upgrade_downgrade() {
        let x = ArcSlice::new(box [1i]);
        let y: WeakSlice<_> = x.downgrade();

        assert_eq!(y.upgrade(), Some(x.clone()));

        drop(x);

        assert!(y.upgrade().is_none())
    }

    #[test]
    fn test_total_cmp() {
        let x = ArcSlice::new(box [1i, 2i, 3i]);
        let y = ArcSlice::new(box [1i, 2i, 3i]);
        let z = ArcSlice::new(box [1i, 2i, 4i]);

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
        let x = ArcSlice::new(box [1.0, f64::NAN]);
        let y = ArcSlice::new(box [1.0, f64::NAN]);
        let z = ArcSlice::new(box [2.0, f64::NAN]);
        let w = ArcSlice::new(box [f64::NAN, 1.0]);
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
        let x = ArcSlice::new(box [1i, 2i]);
        assert_eq!(format!("{}", x), "[1, 2]");

        let y: ArcSlice<int> = ArcSlice::new(box []);
        assert_eq!(format!("{}", y), "[]");
    }

    #[test]
    fn test_slice() {
        let x = ArcSlice::new(box [1i, 2i, 3i]);
        let real = [1, 2, 3];
        for i in range(0, 3 + 1) {
            for j in range(i, 3 + 1) {
                let slice: ArcSlice<_> = x.clone().slice(i, j);
                assert_eq!(&*slice, real.slice(i, j));
            }
            assert_eq!(&*x.clone().slice_to(i), real.slice_to(i));
            assert_eq!(&*x.clone().slice_from(i), real.slice_from(i));
        }
    }


    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Send>() {}

        assert_send::<ArcSlice<u8>>();
        assert_sync::<ArcSlice<u8>>();
        assert_send::<WeakSlice<u8>>();
        assert_sync::<WeakSlice<u8>>();
    }
}
