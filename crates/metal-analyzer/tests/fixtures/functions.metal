#include <metal_stdlib>
#include "types.metal"

using namespace metal;

float4 transform(float4 pos, float scale) {
  return pos * scale;
}

kernel void my_kernel(
    device MyStruct* data [[buffer(0)]],
    const constant MyParams* params [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
  float4 result = transform(data[id].position, params->scale);
  data[id].color = result;
}
