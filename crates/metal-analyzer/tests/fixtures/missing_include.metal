#include <metal_stdlib>
#include "types.metal"
#include "nonexistent.h"

using namespace metal;

kernel void broken_kernel(
    device MyStruct* data [[buffer(0)]],
    const constant MissingType* params [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
  data[id].color = float4(1.0);
}
