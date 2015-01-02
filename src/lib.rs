#![feature(default_type_params, unsafe_destructor, globs)]
#![cfg_attr(not(test), no_std)]

//! Thread-local and thread-safe shared slice types, like `&[T]` but
//! without lifetimes.
//!
//! This library depends only on `alloc` and `core`, so can be used in
//! environments without `std`.
//!
//! # Examples
//!
//! Alice has a long list of numbers which she needs to sum up before
//! she's allowed to enter Wonderland. She's impatient to get there
//! and she has a computer with many cores so she wants to use all of
//! them.
//!
//! Using a `ArcSlice`, she can manually divide up the numbers into
//! chunks and distribute them across some threads.
//!
//! ```rust
//! use shared_slice::arc::ArcSlice;
//! use std::{cmp, rand, sync};
//!
//! // Alice's numbers (the Mad Hatter doesn't care which numbers,
//! // just that they've been summed up).
//! let numbers = range(0u, 10_000)
//!     .map(|_| rand::random::<u64>() % 100)
//!     .collect::<Vec<_>>();
//!
//!
//! const NTHREADS: uint = 10;
//!
//! let numbers = ArcSlice::new(numbers.into_boxed_slice());
//!
//! // number of elements per thread (rounded up)
//! let per_thread = (numbers.len() + NTHREADS - 1) / NTHREADS;
//!
//! let mut futures = range(0, NTHREADS).map(|i| {
//!     // compute the bounds
//!     let lo = i * per_thread;
//!     let hi = cmp::min(numbers.len(), lo + per_thread);
//!
//!     // extract the subsection of the vector that we care about,
//!     // note that the `clone` (which just increases the reference
//!     // counts) is necessary because `ArcSlice::slice` consumes
//!     // the receiver (in order to minimise unnecessary reference
//!     // count modifications).
//!     let my_numbers: ArcSlice<_> = numbers.clone().slice(lo, hi);
//!
//!     // do this part of the sum:
//!     sync::Future::spawn(move || {
//!         my_numbers.iter().fold(0, |a, &b| a + b)
//!     })
//! }).collect::<Vec<sync::Future<u64>>>();
//!
//! // sum up the results from each subsum.
//! let sum = futures.iter_mut().fold(0, |a, b| a + b.get());
//!
//! println!("the sum is {}", sum);
//! ```
//!
//! (NB. `ArcSlice` may become unnecessary for situations like this if
//! [`Send` stops implying
//! `'static`](https://github.com/rust-lang/rfcs/pull/458), since it
//! is likely that one will be able to use conventional borrowed
//! `&[T]` slices directly.)

extern crate alloc;
extern crate core;

pub mod rc;
pub mod arc;
