#pragma once

#if defined(__METAL__)
#define MTL_CONST constant
#else
#define MTL_CONST
#endif

#define FIXTURE_PRAGMA_UNROLL _Pragma("clang loop unroll(full)")
