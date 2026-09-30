//! libtorch's CUDA caching allocator: release what it holds, and read what it holds.
//!
//! Freed CUDA tensors stay reserved by libtorch for reuse. A PPO update uploads a batch of a
//! different size every time, so the reserve grew update after update; on Windows the driver let it
//! spill past the card into shared system memory, and the optimise step slowed from ~8 s to 35-65 s
//! whenever it touched the spilled part (2026-09-22). `tch` 0.22 has no call for this, so a C++ shim
//! (`cuda_cache.cpp`, compiled by `build.rs` only against a CUDA libtorch) provides one.
//!
//! This is the one module in the crate allowed `unsafe` (operator approval, 2026-09-22): two calls
//! into that shim, neither of which lets a C++ exception cross the boundary.

/// Allocator bytes on one device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaMemory {
    /// Held by live tensors.
    pub allocated: i64,
    /// Held by the allocator, live or cached.
    pub reserved: i64,
}

#[cfg(ti4_cuda_cache)]
#[allow(unsafe_code)]
mod shim {
    unsafe extern "C" {
        fn ti4_cuda_empty_cache() -> i32;
        fn ti4_cuda_memory(device: i32, allocated: *mut i64, reserved: *mut i64) -> i32;
    }

    pub fn empty_cache() -> bool {
        // SAFETY: no arguments; the shim catches every C++ exception and returns non-zero.
        unsafe { ti4_cuda_empty_cache() == 0 }
    }

    pub fn memory(device: i32) -> Option<super::CudaMemory> {
        let (mut allocated, mut reserved) = (0_i64, 0_i64);
        // SAFETY: both pointers are to live, writable `i64`s on this stack frame and the shim
        // writes one `int64_t` through each; it catches every C++ exception.
        let status = unsafe { ti4_cuda_memory(device, &raw mut allocated, &raw mut reserved) };
        (status == 0).then_some(super::CudaMemory {
            allocated,
            reserved,
        })
    }
}

/// Whether this build can reach the allocator (a CUDA libtorch and the CUDA toolkit headers).
#[must_use]
pub const fn available() -> bool {
    cfg!(ti4_cuda_cache)
}

/// Give every cached block no tensor is using back to the driver. `false` when this build has no
/// shim or libtorch refused; tensors still in use are never touched either way.
pub fn release_cached() -> bool {
    #[cfg(ti4_cuda_cache)]
    {
        tch::Cuda::is_available() && shim::empty_cache()
    }
    #[cfg(not(ti4_cuda_cache))]
    {
        false
    }
}

/// What the allocator holds on CUDA device `device`, or `None` without a shim or a device.
#[must_use]
pub fn memory(device: usize) -> Option<CudaMemory> {
    #[cfg(ti4_cuda_cache)]
    {
        if !tch::Cuda::is_available() {
            return None;
        }
        shim::memory(i32::try_from(device).ok()?)
    }
    #[cfg(not(ti4_cuda_cache))]
    {
        let _ = device;
        None
    }
}
