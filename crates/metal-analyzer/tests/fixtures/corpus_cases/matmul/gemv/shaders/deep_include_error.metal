#include <metal_stdlib>
using namespace metal;

#include "../../../common/broken_header.h"

kernel void deep_include_error(
    device float* data [[buffer(0)]],
    uint tid [[thread_position_in_grid]]
) {
    data[tid] = broken_value(data[tid]);
}
