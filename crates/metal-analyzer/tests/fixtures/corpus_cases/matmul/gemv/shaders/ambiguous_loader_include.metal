#include <metal_stdlib>
using namespace metal;

#include "loader.h"
#include "../../common/loader.h"
#include "../../common/steel/gemm/loader.h"

kernel void ambiguous_loader_include(
    device float* data [[buffer(0)]],
    uint tid [[thread_position_in_grid]]
) {
    data[tid] = 0.0;
}
