#ifndef FIXTURE_GENERATED_MATMUL_H
#define FIXTURE_GENERATED_MATMUL_H

#ifdef __METAL_VERSION__
#include <metal_stdlib>
using namespace metal;

namespace fixture {

struct GeneratedMatmulParams {
    uint rows;
    uint cols;
};

} // namespace fixture

#else

typedef struct {
    unsigned rows;
    unsigned cols;
} GeneratedMatmulParams;

#endif

#endif
