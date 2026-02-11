#include <metal_stdlib>

using namespace metal;

struct MyParams {
  int width;
  int height;
  float scale;
};

struct MyStruct {
  float4 position;
  float4 color;
};
