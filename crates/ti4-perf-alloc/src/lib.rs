//! Counting `GlobalAlloc` over mimalloc: `static A: Counting = Counting;` then [`counts`] before and after the code to measure.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
// Live heap bytes (requested sizes) and their high-water mark; `reset_peak` restarts the mark at the current live size.
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static INNER: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub struct Counting;

/// Live heap bytes now (sum of the requested sizes of the blocks not yet freed).
#[must_use]
pub fn live() -> u64 {
    LIVE.load(Relaxed)
}

/// The highest live size since the start or the last [`reset_peak`].
#[must_use]
pub fn peak() -> u64 {
    PEAK.load(Relaxed)
}

/// Restart the high-water mark at the current live size.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Relaxed), Relaxed);
}

fn grow(n: u64) {
    let now = LIVE.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(now, Relaxed);
}

/// `(allocations, bytes requested)` since the start of the process.
#[must_use]
pub fn counts() -> (u64, u64) {
    (ALLOCS.load(Relaxed), BYTES.load(Relaxed))
}

// SAFETY: every call is forwarded unchanged to mimalloc; the counters are only read and written atomically.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        grow(l.size() as u64);
        unsafe { INNER.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as u64, Relaxed);
        unsafe { INNER.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(n as u64, Relaxed);
        LIVE.fetch_sub(l.size() as u64, Relaxed);
        grow(n as u64);
        unsafe { INNER.realloc(p, l, n) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        grow(l.size() as u64);
        unsafe { INNER.alloc_zeroed(l) }
    }
}
