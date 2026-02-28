#include <metal_stdlib>

using namespace metal;

float my_min(float a, float b) {
  return a < b ? a : b;
}

#define CALL_BIN(fn, x, y) fn((x), (y))

kernel void macro_kernel(
    device float* out [[buffer(0)]],
    uint id [[thread_position_in_grid]]
) {
  out[id] = CALL_BIN(my_min, 1.0f, 2.0f);
}
