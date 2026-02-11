#include <metal_stdlib>
using namespace metal;

#include "../../../common/utils.h"
#include "../../common/steel/utils.h"
#include "../../common/loader.h"
#include "../../common/steel/gemm/loader.h"
#include "../../common/template_math.h"
#include "../../common/operators.h"
#include "../../common/transforms.h"
#include "../../common/steel/gemm/transforms.h"
#include "../../../generated/matmul.h"

#define MTL_CONST static constant constexpr const

template <typename T>
inline T local_template(T x) {
    return fixture::mix_add<T>(x, x);
}

kernel void gemv_like(
    device float* data [[buffer(0)]],
    constant fixture::Params* params [[buffer(1)]],
    constant fixture::GeneratedMatmulParams* shape [[buffer(2)]],
    uint tid [[thread_position_in_grid]]
) {
    float base = fixture::scale_value(data[tid], params->scale);
    float next = fixture::overloaded(base);
    fixture::ComplexLite lhs = {base, 1.0f};
    fixture::ComplexLite rhs = {next, 2.0f};
    fixture::ComplexLite sum = lhs + rhs;
    fixture::TransformNone none = {};
    steel::TransformNone steel_none = {};
    if (shape->rows > 0 && none.marker == 0 && steel_none.marker == 1) {
        data[tid] = local_template(sum.re);
    }
}
