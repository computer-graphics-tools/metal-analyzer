#pragma once

#ifndef OWNER_ONLY_DEFINE
#error owner_missing_symbol
#endif

inline float owner_scaled(float x) {
    return x * OWNER_ONLY_DEFINE;
}
