#include <metal_stdlib>
using namespace metal;

#include "../../../common/math_ops.h"
#include "../../../common/types.h"

kernel void ref_user_a(
    device float* data [[buffer(0)]],
    constant fixture::Params* params [[buffer(1)]],
    uint tid [[thread_position_in_grid]]
) {
    data[tid] = fixture::shared_mul(data[tid], params->scale);
}
