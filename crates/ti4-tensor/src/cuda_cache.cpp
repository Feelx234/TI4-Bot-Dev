// Two C entry points over libtorch's CUDA caching allocator, for `cuda_cache.rs`.
//
// Neither lets a C++ exception cross into Rust: each catches everything and returns non-zero.
#include <c10/cuda/CUDACachingAllocator.h>

#include <cstdint>

extern "C" {

// Hand every cached block no tensor is using back to the driver.
int ti4_cuda_empty_cache() {
  try {
    c10::cuda::CUDACachingAllocator::emptyCache();
    return 0;
  } catch (...) {
    return 1;
  }
}

// Bytes currently allocated to live tensors, and reserved by the allocator, on `device`.
int ti4_cuda_memory(int device, int64_t* allocated, int64_t* reserved) {
  try {
    const auto stats = c10::cuda::CUDACachingAllocator::getDeviceStats(
        static_cast<c10::DeviceIndex>(device));
    const auto aggregate = static_cast<size_t>(c10::CachingAllocator::StatType::AGGREGATE);
    *allocated = stats.allocated_bytes[aggregate].current;
    *reserved = stats.reserved_bytes[aggregate].current;
    return 0;
  } catch (...) {
    return 1;
  }
}

}  // extern "C"
