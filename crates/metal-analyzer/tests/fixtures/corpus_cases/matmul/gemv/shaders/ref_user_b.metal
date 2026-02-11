#include <metal_stdlib>
using namespace metal;

#include "../../../common/math_ops.h"

kernel void ref_user_b(
    device float* data [[buffer(0)]],
    uint tid [[thread_position_in_grid]]
) {
    data[tid] = fixture::shared_mul(data[tid], 2.0f);
}
