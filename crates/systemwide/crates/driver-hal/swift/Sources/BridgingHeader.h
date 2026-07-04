// Bridging header for SotF HAL Driver
// Exposes the CoreAudio AudioServerPlugIn types to Swift

#ifndef BridgingHeader_h
#define BridgingHeader_h

#include <CoreAudio/AudioServerPlugIn.h>
#include <CoreFoundation/CoreFoundation.h>
#include <mach/mach_time.h>
#include <stdatomic.h>
#include <stdint.h>

static inline uint64_t sotf_atomic_fetch_add_u64(uint64_t *ptr, uint64_t value) {
    return atomic_fetch_add_explicit((_Atomic uint64_t *)ptr, value, memory_order_acq_rel) + value;
}

static inline bool sotf_atomic_compare_exchange_u32(uint32_t *ptr, uint32_t expected, uint32_t desired) {
    return atomic_compare_exchange_strong_explicit(
        (_Atomic uint32_t *)ptr,
        &expected,
        desired,
        memory_order_acq_rel,
        memory_order_acquire
    );
}

static inline uint32_t sotf_atomic_exchange_u32(uint32_t *ptr, uint32_t value) {
    return atomic_exchange_explicit((_Atomic uint32_t *)ptr, value, memory_order_acq_rel);
}

#endif /* BridgingHeader_h */
