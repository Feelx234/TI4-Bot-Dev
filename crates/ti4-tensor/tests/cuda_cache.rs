//! The CUDA cache shim against a real device: releasing frees dropped tensors, never live ones.
//!
//! An integration test, not a unit test: `build.rs` forces `torch_cuda` to load for test targets with
//! a linker argument, and cargo applies that to `tests/`, not to the library's own test harness.

use ti4_tensor::cuda_cache::{available, memory, release_cached};

#[test]
fn releasing_frees_a_dropped_tensor_and_keeps_a_live_one() {
    if !available() {
        assert!(!release_cached());
        assert_eq!(memory(0), None);
        return;
    }
    // A shim is only compiled against a CUDA libtorch, so a missing device is a failure here,
    // not a skip.
    assert!(
        tch::Cuda::is_available(),
        "the CUDA shim is built but no device is visible"
    );
    let device = tch::Device::Cuda(0);
    let live = tch::Tensor::ones([1 << 20], (tch::Kind::Float, device));
    let dropped = tch::Tensor::ones([1 << 24], (tch::Kind::Float, device));
    drop(dropped);
    let before = memory(0).expect("stats");
    assert!(
        before.reserved >= before.allocated + (1 << 26),
        "{before:?}"
    );
    assert!(release_cached());
    let after = memory(0).expect("stats");
    eprintln!("cuda cache: {before:?} -> {after:?}");
    assert_eq!(
        after.allocated, before.allocated,
        "a live tensor was released"
    );
    assert!(after.reserved < before.reserved, "{before:?} -> {after:?}");
    assert!((live.sum(tch::Kind::Float).double_value(&[]) - f64::from(1 << 20)).abs() < 1.0);
}
