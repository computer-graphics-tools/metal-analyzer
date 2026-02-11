#include <metal_stdlib>
using namespace metal;

#define OWNER_ONLY_DEFINE 2.0f
#include "../../common/problematic_owner_only.h"

kernel void owner_context(
    device float* data [[buffer(0)]],
    uint tid [[thread_position_in_grid]]
) {
    data[tid] = owner_scaled(data[tid]);
}
