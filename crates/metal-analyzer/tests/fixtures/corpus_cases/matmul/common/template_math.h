#pragma once

#include <metal_stdlib>
using namespace metal;

namespace fixture {

template <typename T>
struct TensorRef {
    device T* data;
    uint stride;

    device T& at(uint i) {
        return data[i * stride];
    }
};

template <typename T>
inline T mix_add(T a, T b) {
    return a + b;
}

inline int overloaded(int x) {
    return x + 1;
}

inline float overloaded(float x) {
    return x + 1.0f;
}

struct Ops {
    static inline float apply(float x) {
        return overloaded(x);
    }
};

} // namespace fixture
