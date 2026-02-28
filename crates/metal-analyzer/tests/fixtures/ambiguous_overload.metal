#include <metal_stdlib>

using namespace metal;

float overload(float x) {
  return x;
}

int overload(int x) {
  return x;
}

kernel void overload_kernel(
    device float* out [[buffer(0)]],
    uint id [[thread_position_in_grid]]
) {
  out[id] = overload(1.0f);
}
