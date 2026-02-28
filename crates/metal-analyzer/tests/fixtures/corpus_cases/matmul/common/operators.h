#pragma once

namespace fixture {

struct ComplexLite {
    float re;
    float im;
};

inline ComplexLite operator+(ComplexLite lhs, ComplexLite rhs) {
    return {lhs.re + rhs.re, lhs.im + rhs.im};
}

} // namespace fixture
