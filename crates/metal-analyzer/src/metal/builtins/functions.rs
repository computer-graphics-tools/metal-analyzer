use crate::metal::builtins::types::BuiltinEntry;

pub(crate) fn add_scalar_types(entries: &mut Vec<BuiltinEntry>) {
    for t in &[
        "bool",
        "char",
        "uchar",
        "short",
        "ushort",
        "int",
        "uint",
        "half",
        "float",
        "size_t",
        "ptrdiff_t",
        "int8_t",
        "int16_t",
        "int32_t",
        "int64_t",
        "uint8_t",
        "uint16_t",
        "uint32_t",
        "uint64_t",
        "void",
        "bfloat",
        "bfloat16_t",
    ] {
        entries.push(BuiltinEntry::typ(t, "Scalar type"));
    }
}

pub(crate) fn add_vector_types(entries: &mut Vec<BuiltinEntry>) {
    for base in &["bool", "char", "uchar", "short", "ushort", "int", "uint", "half", "float", "bfloat"] {
        for n in 2..=4 {
            let label = format!("{base}{n}");
            entries.push(BuiltinEntry::typ(&label, &format!("Vector type ({n} components of {base})")));
        }
    }
}

pub(crate) fn add_matrix_types(entries: &mut Vec<BuiltinEntry>) {
    for base in &["half", "float", "bfloat"] {
        for c in 2..=4 {
            for r in 2..=4 {
                let label = format!("{base}{c}x{r}");
                entries.push(BuiltinEntry::typ(&label, &format!("Matrix type ({c} columns, {r} rows of {base})")));
            }
        }
    }
}

pub(crate) fn add_texture_types(entries: &mut Vec<BuiltinEntry>) {
    let textures = [
        "texture1d",
        "texture1d_array",
        "texture2d",
        "texture2d_array",
        "texture2d_ms",
        "texture2d_ms_array",
        "texture3d",
        "texturecube",
        "texturecube_array",
        "depth2d",
        "depth2d_array",
        "depth2d_ms",
        "depth2d_ms_array",
        "depthcube",
        "depthcube_array",
        "texture_buffer",
    ];
    for t in &textures {
        entries.push(BuiltinEntry::typ(t, "Texture object type"));
    }
}

pub(crate) fn add_sampler_types(entries: &mut Vec<BuiltinEntry>) {
    entries.push(BuiltinEntry::typ("sampler", "Sampler object for texture sampling"));
    entries.push(BuiltinEntry::typ("const_sampler", "Compile-time constant sampler (Metal 2.0+)"));
}

pub(crate) fn add_atomic_types(entries: &mut Vec<BuiltinEntry>) {
    for t in &["atomic_int", "atomic_uint", "atomic_bool", "atomic_float"] {
        entries.push(BuiltinEntry::typ(t, "Atomic type"));
    }
}

pub(crate) fn add_packed_types(entries: &mut Vec<BuiltinEntry>) {
    for base in &["char", "uchar", "short", "ushort", "int", "uint", "half", "float"] {
        for n in 2..=4 {
            let label = format!("packed_{base}{n}");
            entries.push(BuiltinEntry::typ(&label, &format!("Packed vector type ({n} components of {base})")));
        }
    }
}

pub(crate) fn add_math_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "Math";
    let funcs = [
        ("acos", "T acos(T x)", "Arc cosine of x"),
        ("acosh", "T acosh(T x)", "Inverse hyperbolic cosine"),
        ("asin", "T asin(T x)", "Arc sine of x"),
        ("asinh", "T asinh(T x)", "Inverse hyperbolic sine"),
        ("atan", "T atan(T y_over_x)", "Arc tangent"),
        ("atan2", "T atan2(T y, T x)", "Arc tangent of y/x using signs to determine quadrant"),
        ("atanh", "T atanh(T x)", "Inverse hyperbolic tangent"),
        ("ceil", "T ceil(T x)", "Round x up to integer"),
        ("copysign", "T copysign(T x, T y)", "Return x with the sign of y"),
        ("cos", "T cos(T x)", "Cosine of x"),
        ("cosh", "T cosh(T x)", "Hyperbolic cosine"),
        ("cospi", "T cospi(T x)", "Cosine of x * pi"),
        ("exp", "T exp(T x)", "Exponential base e"),
        ("exp2", "T exp2(T x)", "Exponential base 2"),
        ("exp10", "T exp10(T x)", "Exponential base 10"),
        ("fabs", "T fabs(T x)", "Absolute value (float)"),
        ("abs", "T abs(T x)", "Absolute value (integer/float)"),
        ("fdim", "T fdim(T x, T y)", "Positive difference x - y"),
        ("floor", "T floor(T x)", "Round x down to integer"),
        ("fma", "T fma(T a, T b, T c)", "Fused multiply-add: a * b + c"),
        ("fmax", "T fmax(T x, T y)", "Maximum of x and y (floating point)"),
        ("fmin", "T fmin(T x, T y)", "Minimum of x and y (floating point)"),
        ("fmod", "T fmod(T x, T y)", "Floating point remainder of x / y"),
        ("fract", "T fract(T x)", "Fractional part of x (x - floor(x))"),
        ("frexp", "T frexp(T x, thread int &exp)", "Split float"),
        ("ilogb", "int ilogb(T x)", "Integer binary logarithm"),
        ("ldexp", "T ldexp(T x, int k)", "Multiply x by 2 to the power k"),
        ("log", "T log(T x)", "Natural logarithm"),
        ("log2", "T log2(T x)", "Base-2 logarithm"),
        ("log10", "T log10(T x)", "Base-10 logarithm"),
        ("modf", "T modf(T x, thread T &iptr)", "Decompose x into integral and fractional parts"),
        ("nan", "T nan(uint nancode)", "Generate a quiet NaN with nancode"),
        ("nextafter", "T nextafter(T x, T y)", "Next representable value after x towards y"),
        ("pow", "T pow(T x, T y)", "x to the power y"),
        ("pown", "T pown(T x, int y)", "x to the power y (integer y)"),
        ("powr", "T powr(T x, T y)", "x to the power y (x >= 0)"),
        ("remainder", "T remainder(T x, T y)", "IEEE 754 floating point remainder"),
        ("rint", "T rint(T x)", "Round to nearest integer (current rounding mode)"),
        ("round", "T round(T x)", "Round to nearest integer (halfway cases away from zero)"),
        ("rsqrt", "T rsqrt(T x)", "Reciprocal square root (1 / sqrt(x))"),
        ("sign", "T sign(T x)", "Sign of x (-1, 0, or 1)"),
        ("sin", "T sin(T x)", "Sine of x"),
        ("sincos", "void sincos(T x, thread T &s, thread T &c)", "Sine and cosine of x"),
        ("sinh", "T sinh(T x)", "Hyperbolic sine"),
        ("sinpi", "T sinpi(T x)", "Sine of x * pi"),
        ("sqrt", "T sqrt(T x)", "Square root of x"),
        ("tan", "T tan(T x)", "Tangent of x"),
        ("tanh", "T tanh(T x)", "Hyperbolic tangent"),
        ("tanpi", "T tanpi(T x)", "Tangent of x * pi"),
        ("trunc", "T trunc(T x)", "Round x towards zero"),
        ("clamp", "T clamp(T x, T min, T max)", "Clamp x between min and max"),
        ("mix", "T mix(T x, T y, T a)", "Linear interpolation between x and y by a"),
        ("step", "T step(T edge, T x)", "Returns 0.0 if x < edge, else 1.0"),
        ("smoothstep", "T smoothstep(T edge0, T edge1, T x)", "Hermite interpolation between edge0 and edge1"),
        ("isfinite", "bool isfinite(T x)", "Test if x is finite"),
        ("isinf", "bool isinf(T x)", "Test if x is infinite"),
        ("isnan", "bool isnan(T x)", "Test if x is Not-a-Number (NaN)"),
        ("isnormal", "bool isnormal(T x)", "Test if x is a normal floating-point value"),
    ];

    for (name, detail, doc) in &funcs {
        entries.push(BuiltinEntry::func(name, detail, doc, cat));
    }
}

pub(crate) fn add_geometric_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "Geometric";
    let funcs = [
        ("cross", "T cross(T x, T y)", "Cross product of two 3-component vectors"),
        ("distance", "T distance(T x, T y)", "Distance between two points"),
        ("dot", "T dot(T x, T y)", "Dot product of two vectors"),
        ("faceforward", "T faceforward(T N, T I, T Nref)", "Orient a vector to point away from a surface"),
        ("length", "T length(T x)", "Length (magnitude) of a vector"),
        ("normalize", "T normalize(T x)", "Normalize a vector to length 1"),
        ("reflect", "T reflect(T I, T N)", "Calculate reflection direction"),
        ("refract", "T refract(T I, T N, T eta)", "Calculate refraction direction"),
    ];

    for (name, detail, doc) in &funcs {
        entries.push(BuiltinEntry::func(name, detail, doc, cat));
    }
}

pub(crate) fn add_relational_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "Relational";
    let funcs = [
        ("all", "bool all(boolN x)", "True if all components are true"),
        ("any", "bool any(boolN x)", "True if any component is true"),
        ("select", "T select(T a, T b, bool c)", "Select a or b based on c (component-wise)"),
    ];
    for (name, detail, doc) in &funcs {
        entries.push(BuiltinEntry::func(name, detail, doc, cat));
    }
}

pub(crate) fn add_texture_functions(_entries: &mut Vec<BuiltinEntry>) {
    let _cat = "Texture";
    // Note: Most texture operations are methods on the texture types (e.g. tex.sample()),
    // handled by `member_completions`. However, some global functions exist or are
    // aliases in older Metal versions.
    // We add common texture types here as "functions" for completion if the user
    // treats them as constructors, but mainly texture/sampler types are added in
    // add_texture_types.

    // Global functions that might be useful:
    // (None significant in modern Metal that aren't methods)

    // Instead, we add helper functions often associated with textures
    // like `sampler` constructors are handled by `add_sampler_types` as types.

    // We'll leave this empty or minimal for now as most are member functions.
    // Just adding a placeholder to match structure.
}

pub(crate) fn add_synchronization_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "Synchronization";
    entries.push(BuiltinEntry::func(
        "threadgroup_barrier",
        "void threadgroup_barrier(mem_flags flags)",
        "Wait for all threads in the threadgroup to reach this point",
        cat,
    ));
    entries.push(BuiltinEntry::func(
        "simdgroup_barrier",
        "void simdgroup_barrier(mem_flags flags)",
        "Wait for all threads in the SIMD group to reach this point",
        cat,
    ));
    entries.push(BuiltinEntry::func(
        "device_memory_barrier_with_hint",
        "void device_memory_barrier_with_hint(mem_flags flags)",
        "Memory barrier for device memory",
        cat,
    ));
    entries.push(BuiltinEntry::func(
        "threadgroup_memory_barrier_with_hint",
        "void threadgroup_memory_barrier_with_hint(mem_flags flags)",
        "Memory barrier for threadgroup memory",
        cat,
    ));
}

pub(crate) fn add_simd_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "SIMD";
    let funcs = [
        ("simd_sum", "T simd_sum(T data)", "Sum of data across the SIMD group"),
        ("simd_product", "T simd_product(T data)", "Product of data across the SIMD group"),
        ("simd_min", "T simd_min(T data)", "Minimum of data across the SIMD group"),
        ("simd_max", "T simd_max(T data)", "Maximum of data across the SIMD group"),
        ("simd_prefix_exclusive_sum", "T simd_prefix_exclusive_sum(T data)", "Exclusive prefix sum"),
        ("simd_prefix_inclusive_sum", "T simd_prefix_inclusive_sum(T data)", "Inclusive prefix sum"),
        ("simd_prefix_exclusive_product", "T simd_prefix_exclusive_product(T data)", "Exclusive prefix product"),
        ("simd_prefix_inclusive_product", "T simd_prefix_inclusive_product(T data)", "Inclusive prefix product"),
        ("simd_shuffle", "T simd_shuffle(T data, ushort lane)", "Shuffle data from another lane"),
        ("simd_shuffle_down", "T simd_shuffle_down(T data, ushort delta)", "Shuffle data from a lower lane"),
        ("simd_shuffle_up", "T simd_shuffle_up(T data, ushort delta)", "Shuffle data from a higher lane"),
        ("simd_shuffle_xor", "T simd_shuffle_xor(T data, ushort mask)", "Shuffle data using XOR on lane ID"),
        ("simd_broadcast", "T simd_broadcast(T data, ushort lane)", "Broadcast data from one lane to all"),
        ("simd_ballot", "ulong simd_ballot(bool predicate)", "Bitmask of lanes where predicate is true"),
        ("quad_broadcast", "T quad_broadcast(T data, ushort lane)", "Broadcast within a quad"),
        ("quad_shuffle_up", "T quad_shuffle_up(T data, ushort delta)", "Shuffle up within a quad"),
        ("quad_shuffle_down", "T quad_shuffle_down(T data, ushort delta)", "Shuffle down within a quad"),
        ("quad_shuffle_xor", "T quad_shuffle_xor(T data, ushort mask)", "Shuffle XOR within a quad"),
    ];

    for (name, detail, doc) in &funcs {
        entries.push(BuiltinEntry::func(name, detail, doc, cat));
    }
}

pub(crate) fn add_atomic_functions(entries: &mut Vec<BuiltinEntry>) {
    let cat = "Atomic";
    let funcs = [
        (
            "atomic_store_explicit",
            "void atomic_store_explicit(volatile device A* obj, T des, memory_order order)",
            "Atomic store",
        ),
        ("atomic_load_explicit", "T atomic_load_explicit(volatile device A* obj, memory_order order)", "Atomic load"),
        (
            "atomic_exchange_explicit",
            "T atomic_exchange_explicit(volatile device A* obj, T des, memory_order order)",
            "Atomic exchange",
        ),
        (
            "atomic_compare_exchange_weak_explicit",
            "bool atomic_compare_exchange_weak_explicit(volatile device A* obj, device T* expected, T desired, memory_order succ, memory_order fail)",
            "Atomic CAS (weak)",
        ),
        (
            "atomic_compare_exchange_strong_explicit",
            "bool atomic_compare_exchange_strong_explicit(volatile device A* obj, device T* expected, T desired, memory_order succ, memory_order fail)",
            "Atomic CAS (strong)",
        ),
        (
            "atomic_fetch_add_explicit",
            "T atomic_fetch_add_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch add",
        ),
        (
            "atomic_fetch_sub_explicit",
            "T atomic_fetch_sub_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch sub",
        ),
        (
            "atomic_fetch_or_explicit",
            "T atomic_fetch_or_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch or",
        ),
        (
            "atomic_fetch_xor_explicit",
            "T atomic_fetch_xor_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch xor",
        ),
        (
            "atomic_fetch_and_explicit",
            "T atomic_fetch_and_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch and",
        ),
        (
            "atomic_fetch_min_explicit",
            "T atomic_fetch_min_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch min",
        ),
        (
            "atomic_fetch_max_explicit",
            "T atomic_fetch_max_explicit(volatile device A* obj, T operand, memory_order order)",
            "Atomic fetch max",
        ),
    ];

    for (name, detail, doc) in &funcs {
        entries.push(BuiltinEntry::func(name, detail, doc, cat));
    }
}

pub(crate) fn add_attributes(entries: &mut Vec<BuiltinEntry>) {
    let attrs = [
        ("buffer", "[[buffer(n)]]", "Assigns a buffer to an index in the buffer argument table."),
        ("texture", "[[texture(n)]]", "Assigns a texture to an index in the texture argument table."),
        ("sampler", "[[sampler(n)]]", "Assigns a sampler to an index in the sampler argument table."),
        ("thread_position_in_grid", "[[thread_position_in_grid]]", "The position of the thread in the grid."),
        (
            "thread_position_in_threadgroup",
            "[[thread_position_in_threadgroup]]",
            "The position of the thread in the threadgroup.",
        ),
        (
            "thread_index_in_threadgroup",
            "[[thread_index_in_threadgroup]]",
            "The linear index of the thread in the threadgroup.",
        ),
        (
            "thread_index_in_simdgroup",
            "[[thread_index_in_simdgroup]]",
            "The linear index of the thread in the SIMD group.",
        ),
        (
            "threadgroup_position_in_grid",
            "[[threadgroup_position_in_grid]]",
            "The position of the threadgroup in the grid.",
        ),
        ("threads_per_grid", "[[threads_per_grid]]", "The size of the grid in threads."),
        ("threads_per_threadgroup", "[[threads_per_threadgroup]]", "The size of the threadgroup in threads."),
        ("simd_position_in_grid", "[[simd_position_in_grid]]", "The position of the SIMD group in the grid."),
        (
            "simdgroup_index_in_threadgroup",
            "[[simdgroup_index_in_threadgroup]]",
            "The index of the SIMD group in the threadgroup.",
        ),
        ("position", "[[position]]", "Vertex position (graphics) or pixel position (fragment)."),
        ("vertex_id", "[[vertex_id]]", "The current vertex index."),
        ("instance_id", "[[instance_id]]", "The current instance index."),
        ("primitive_id", "[[primitive_id]]", "The current primitive index."),
        ("point_size", "[[point_size]]", "Point size for point primitives."),
        ("color", "[[color(n)]]", "Output color attachment index."),
        ("raster_order_group", "[[raster_order_group(n)]]", "Raster order group index."),
        ("early_fragment_tests", "[[early_fragment_tests]]", "Force early fragment tests."),
    ];

    for (label, snippet, doc) in &attrs {
        entries.push(BuiltinEntry::attr(label, doc, Some(snippet)));
        // Also add the full attribute string for direct matching
        entries.push(BuiltinEntry::attr(snippet, doc, None));
    }
}

pub(crate) fn add_sampler_constants(entries: &mut Vec<BuiltinEntry>) {
    let consts = [
        ("coord::normalized", "Normalized texture coordinates (0.0 to 1.0)"),
        ("coord::pixel", "Unnormalized pixel texture coordinates"),
        ("address::clamp_to_edge", "Clamp texture coordinates to the edge"),
        ("address::clamp_to_zero", "Clamp texture coordinates to zero (transparent black)"),
        ("address::clamp_to_border", "Clamp texture coordinates to border color"),
        ("address::repeat", "Repeat texture coordinates"),
        ("address::mirrored_repeat", "Mirror repeat texture coordinates"),
        ("filter::nearest", "Nearest-neighbor filtering"),
        ("filter::linear", "Linear filtering"),
        ("mip_filter::none", "No mipmap filtering"),
        ("mip_filter::nearest", "Nearest-neighbor mipmap filtering"),
        ("mip_filter::linear", "Linear mipmap filtering"),
        ("compare_func::less", "Pass if value < reference"),
        ("compare_func::less_equal", "Pass if value <= reference"),
        ("compare_func::greater", "Pass if value > reference"),
        ("compare_func::greater_equal", "Pass if value >= reference"),
        ("compare_func::equal", "Pass if value == reference"),
        ("compare_func::not_equal", "Pass if value != reference"),
        ("compare_func::always", "Always pass"),
        ("compare_func::never", "Never pass"),
        ("access::read", "Read-only access"),
        ("access::write", "Write-only access"),
        ("access::read_write", "Read-write access"),
    ];

    for (label, doc) in &consts {
        entries.push(BuiltinEntry::constant(label, "enum constant", doc));
    }
}

pub(crate) fn add_snippets(entries: &mut Vec<BuiltinEntry>) {
    entries.push(BuiltinEntry::snippet(
        "kernel",
        "Kernel Function",
        "kernel void ${1:name}(device ${2:type}* ${3:buffer} [[buffer(0)]], uint ${4:id} [[thread_position_in_grid]]) {\n\t$0\n}",
    ));
    entries.push(BuiltinEntry::snippet(
        "vertex",
        "Vertex Function",
        "vertex ${1:VertexOut} ${2:name}(uint ${3:vertexID} [[vertex_id]], constant ${4:Uniforms}& ${5:uniforms} [[buffer(0)]]) {\n\t$0\n}",
    ));
    entries.push(BuiltinEntry::snippet(
        "fragment",
        "Fragment Function",
        "fragment float4 ${1:name}(${2:VertexOut} ${3:in} [[stage_in]]) {\n\treturn float4(1.0);\n}",
    ));
}

pub(crate) fn add_raytracing_types(entries: &mut Vec<BuiltinEntry>) {
    let types = [
        "ray",
        "ray_tracing::intersection_result",
        "ray_tracing::intersector",
        "ray_tracing::instance_acceleration_structure",
        "ray_tracing::primitive_acceleration_structure",
        "ray_tracing::ray_tracing_acceleration_structure",
    ];
    for t in &types {
        entries.push(BuiltinEntry::typ(t, "Raytracing type"));
    }
}

pub(crate) fn add_misc_types(entries: &mut Vec<BuiltinEntry>) {
    // Other useful types that don't fit categories perfectly
    let types = ["render_grid_type", "object_grid_type", "mem_flags", "thread_scope"];
    for t in &types {
        entries.push(BuiltinEntry::typ(t, "Metal type"));
    }
}

pub(crate) fn add_builtin_constants(entries: &mut Vec<BuiltinEntry>) {
    let consts = [
        ("INFINITY", "float", "Infinity"),
        ("NAN", "float", "Not a Number"),
        ("M_E_F", "float", "e"),
        ("M_LOG2E_F", "float", "log2(e)"),
        ("M_LOG10E_F", "float", "log10(e)"),
        ("M_LN2_F", "float", "ln(2)"),
        ("M_LN10_F", "float", "ln(10)"),
        ("M_PI_F", "float", "pi"),
        ("M_PI_2_F", "float", "pi / 2"),
        ("M_PI_4_F", "float", "pi / 4"),
        ("M_1_PI_F", "float", "1 / pi"),
        ("M_2_PI_F", "float", "2 / pi"),
        ("M_2_SQRTPI_F", "float", "2 / sqrt(pi)"),
        ("M_SQRT2_F", "float", "sqrt(2)"),
        ("M_SQRT1_2_F", "float", "1 / sqrt(2)"),
        ("MAXFLOAT", "float", "Maximum finite float value"),
        ("HUGE_VALF", "float", "Huge float value"),
        ("INT_MAX", "int", "Maximum int value"),
        ("INT_MIN", "int", "Minimum int value"),
        ("UINT_MAX", "uint", "Maximum uint value"),
        ("CHAR_BIT", "int", "Number of bits in a char"),
        ("mem_none", "mem_flags", "Memory barrier flag: no memory class selected"),
        ("mem_device", "mem_flags", "Memory barrier flag: synchronize device memory"),
        ("mem_threadgroup", "mem_flags", "Memory barrier flag: synchronize threadgroup memory"),
        ("mem_threadgroup_imageblock", "mem_flags", "Memory barrier flag: synchronize threadgroup imageblock memory"),
    ];

    for (name, detail, doc) in &consts {
        entries.push(BuiltinEntry::constant(name, detail, doc));
    }
}
