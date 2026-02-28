; highlights.scm — Metal Shading Language (using tree-sitter-cpp grammar)
; Enhanced with Metal-specific patterns for shader programming

; Comments
(comment) @comment

; Default identifiers (kept near top so later specific captures override it)
(identifier) @variable

; Strings
(string_literal) @string
(raw_string_literal) @string
(system_lib_string) @string
(char_literal) @string
(escape_sequence) @string.escape

; Numbers
(number_literal) @number

; Constants
(true) @boolean
(false) @boolean
(null) @constant.builtin
("nullptr") @constant.builtin
(this) @variable.builtin

; Metal-specific: Built-in constants
(identifier) @constant.builtin
  (#match? @constant.builtin "^(INFINITY|NAN|MAXFLOAT|HUGE_VALF|M_PI_F|M_E_F|FLT_MAX|FLT_MIN|INT_MAX|INT_MIN|UINT_MAX|CHAR_BIT)$")

; C/C++ style macro constants
((identifier) @constant.builtin
  (#match? @constant.builtin "^_*[A-Z][A-Z\\d_]*$"))

; Types - Base
(type_identifier) @type
(primitive_type) @type
(sized_type_specifier) @type
(auto) @type
type: (primitive_type) @type.builtin

; Metal-specific: Scalar types
(primitive_type) @type.builtin
  (#match? @type.builtin "^(half|bfloat|bfloat16_t|metal::half|metal::bfloat)$")

; Metal-specific: Vector types (float2, float3, float4, etc.)
(type_identifier) @type.builtin
  (#match? @type.builtin "^(float[234]|half[234]|int[234]|uint[234]|short[234]|ushort[234]|char[234]|uchar[234]|bool[234]|long[234]|ulong[234]|bfloat[234])$")

; Metal-specific: Matrix types
(type_identifier) @type.builtin
  (#match? @type.builtin "^(float[234]x[234]|half[234]x[234])$")

; Metal-specific: Packed types
(type_identifier) @type.builtin
  (#match? @type.builtin "^packed_(float[234]|half[234]|int[234]|uint[234])$")

; Metal-specific: Texture types
(type_identifier) @type.builtin
  (#match? @type.builtin "^(texture[123]d|texture[123]d_array|texturecube|texturecube_array|texture2d_ms|texture2d_ms_array|depth2d|depth2d_array|depth2d_ms|depth2d_ms_array|depthcube|depthcube_array)$")

; Metal-specific: Sampler and buffer types
(type_identifier) @type.builtin
  (#match? @type.builtin "^(sampler|device|threadgroup|constant|thread|ray_data|primitive_acceleration_structure|instance_acceleration_structure|intersection_result|intersector|imageblock|imageblock_slice)$")

; Preprocessor
(preproc_directive) @preproc
[
  "#define"
  "#elif"
  "#elifdef"
  "#elifndef"
  "#else"
  "#endif"
  "#if"
  "#ifdef"
  "#ifndef"
  "#include"
] @preproc
(preproc_def
  name: (identifier) @constant)
(preproc_ifdef
  name: (identifier) @constant)
(preproc_function_def
  name: (identifier) @function.special)

; Namespaces (Metal uses metal namespace)
(namespace_identifier) @namespace
((namespace_identifier) @type
 (#match? @type "^[A-Z]"))

; C++ concepts/modules (parsed by cpp grammar, useful for shared headers)
(concept_definition
    name: (identifier) @concept)

(requires_clause
    constraint: (template_type
        name: (type_identifier) @concept))

(module_name
  (identifier) @module)

(module_declaration
  name: (module_name
    (identifier) @module))

(import_declaration
  name: (module_name
    (identifier) @module))

(import_declaration
  partition: (module_partition
    (module_name
      (identifier) @module)))

; Functions — declarations
(function_declarator
  declarator: (identifier) @function)
(function_declarator
  declarator: (field_identifier) @function)
(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))
; Fallback for macro-qualified and uncommon declarator forms
(function_declarator
  declarator: (_) @function)

; Functions — calls
(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function))
(call_expression
  function: (qualified_identifier
    name: (identifier) @function))

; Richer qualified call matching (parity with C++ query depth)
(call_expression
  (qualified_identifier
    (identifier) @function.call))

(call_expression
  (qualified_identifier
    (qualified_identifier
      (identifier) @function.call)))

(call_expression
  (qualified_identifier
    (qualified_identifier
      (qualified_identifier
        (identifier) @function.call))))

((qualified_identifier
  (qualified_identifier
    (qualified_identifier
      (qualified_identifier
        (identifier) @function.call)))) @_parent
  (#has-ancestor? @_parent call_expression))

; Metal-specific: Built-in math functions
(call_expression
  function: (identifier) @function.builtin
    (#match? @function.builtin "^(abs|acos|acosh|asin|asinh|atan|atan2|atanh|ceil|clamp|cos|cosh|exp|exp2|exp10|fabs|floor|fma|fmax|fmin|fmod|fract|frexp|ldexp|log|log2|log10|max|min|mix|modf|pow|round|rsqrt|sign|sin|sinh|smoothstep|sqrt|step|tan|tanh|trunc)$"))

; Metal-specific: Geometric functions
(call_expression
  function: (identifier) @function.builtin
    (#match? @function.builtin "^(cross|distance|dot|faceforward|length|normalize|reflect|refract)$"))

; Metal-specific: Relational functions
(call_expression
  function: (identifier) @function.builtin
    (#match? @function.builtin "^(all|any|select|isfinite|isinf|isnan|isnormal|signbit)$"))

; Metal-specific: SIMD and threadgroup functions
(call_expression
  function: (identifier) @function.builtin
    (#match? @function.builtin "^(simd_shuffle|simd_shuffle_down|simd_shuffle_up|simd_sum|simd_product|simd_min|simd_max|simd_prefix_.*|simd_broadcast|quad_broadcast|quad_shuffle|quad_shuffle_down|threadgroup_barrier|simdgroup_barrier)$"))

; Metal-specific: Atomic functions
(call_expression
  function: (identifier) @function.builtin
    (#match? @function.builtin "^atomic_(load|store|exchange|compare_exchange|fetch_add|fetch_sub|fetch_and|fetch_or|fetch_xor|fetch_min|fetch_max)"))

; Templates
(template_function
  name: (identifier) @function)
(template_method
  name: (field_identifier) @function)

; Special function-like symbols
(operator_name
  (identifier)? @operator) @function
(operator_name
  "<=>" @operator.spaceship)
(destructor_name (identifier) @function)

; Fields
(field_identifier) @property

; Metal-specific: Texture/sampler methods
(field_identifier) @function.builtin
  (#match? @function.builtin "^(sample|read|write|gather|get_width|get_height|get_depth|get_num_samples|get_num_mip_levels|get_array_size|fence)$")

; Labels
(statement_identifier) @label
("static_assert") @function.builtin

; Keywords - Standard C++
[
  "alignas"
  "alignof"
  "class"
  "concept"
  "const"
  "consteval"
  "constexpr"
  "constinit"
  "decltype"
  "delete"
  "enum"
  "explicit"
  "export"
  "extern"
  "final"
  "friend"
  "import"
  "inline"
  "module"
  "mutable"
  "namespace"
  "new"
  "noexcept"
  "operator"
  "override"
  "private"
  "protected"
  "public"
  "requires"
  "sizeof"
  "static"
  "struct"
  "template"
  "thread_local"
  "typedef"
  "typename"
  "union"
  "using"
  "virtual"
  "volatile"
  (storage_class_specifier)
  (type_qualifier)
] @keyword

; Control-flow keywords (allows richer theme styling)
[
  "break"
  "case"
  "catch"
  "co_await"
  "co_return"
  "co_yield"
  "continue"
  "default"
  "do"
  "else"
  "for"
  "goto"
  "if"
  "return"
  "switch"
  "throw"
  "try"
  "while"
] @keyword.control

; Metal-specific: Shader stage qualifiers
(identifier) @keyword
  (#match? @keyword "^(kernel|vertex|fragment|mesh|object)$")

; Metal-specific: Address space qualifiers
(type_qualifier) @keyword.storage
  (#match? @keyword.storage "^(device|threadgroup|constant|thread|ray_data)$")

; Capture qualifiers even when parser emits them as identifiers
(identifier) @keyword.storage
  (#match? @keyword.storage "^(device|threadgroup|constant|thread|threadgroup_imageblock|threadgroup_imageblock_data)$")

; Metal-specific: Function qualifiers
(identifier) @keyword
  (#match? @keyword "^(visible|inline|constexpr|consteval)$")

; Metal-specific: Access qualifiers
(identifier) @keyword
  (#match? @keyword "^(access|sample|read|write|read_write)$")

; Common Metal built-in attribute semantics
(identifier) @variable.builtin
  (#match? @variable.builtin "^(thread_position_in_grid|threadgroup_position_in_grid|threads_per_grid|threads_per_threadgroup|thread_index_in_threadgroup|thread_index_in_simdgroup|simdgroup_index_in_threadgroup|position|vertex_id|instance_id|primitive_id|sample_id)$")

; Preprocessor helper builtin
((identifier) @function.builtin
  (#eq? @function.builtin "_Pragma"))

; Operators
[
  "="
  "+"
  "-"
  "*"
  "/"
  "%"
  "&"
  "|"
  "^"
  "~"
  "!"
  "<"
  ">"
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "=="
  "!="
  "<="
  ">="
  "&&"
  "||"
  "<<"
  ">>"
  "++"
  "--"
  "->"
  "::"
  "?"
  ":"
  ".*"
  "->*"
  "and"
  "and_eq"
  "bitand"
  "bitor"
  "compl"
  "not"
  "not_eq"
  "or"
  "or_eq"
  "xor"
  "xor_eq"
] @operator
"<=>" @operator.spaceship
(binary_expression
  operator: "<=>" @operator.spaceship)
(conditional_expression ":" @operator)
(user_defined_literal (literal_suffix) @operator)

; Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

";" @punctuation.delimiter
"," @punctuation.delimiter
(raw_string_delimiter) @punctuation.delimiter

; Metal-specific: Attribute brackets [[ ]]
; Note: tree-sitter-cpp parses these as attribute_specifier nodes
(attribute) @attribute
(attribute_specifier) @attribute
(attribute_specifier
  (argument_list
    (identifier) @attribute))
(attribute
  prefix: (identifier) @attribute)
(attribute
  name: (identifier) @attribute)
