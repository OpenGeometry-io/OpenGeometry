use crate::exceptions::ListSpec;

pub(crate) const PORTED_LONG_FUNCTIONS: ListSpec = ListSpec {
    file: "ported_long_functions.txt",
    key_fields: 2,
    measured: true,
    frozen: &[],
};

pub(crate) const UPWARD_IMPORTS: ListSpec = ListSpec {
    file: "upward_imports.txt",
    key_fields: 2,
    measured: false,
    frozen: &[],
};

pub(crate) const MODULE_CYCLES: ListSpec = ListSpec {
    file: "module_cycles.txt",
    key_fields: 1,
    measured: false,
    frozen: &[],
};

pub(crate) const MUTUAL_IMPORTS: ListSpec = ListSpec {
    file: "mutual_imports.txt",
    key_fields: 2,
    measured: false,
    frozen: &[],
};

pub(crate) const SIBLING_CYCLES: ListSpec = ListSpec {
    file: "sibling_cycles.txt",
    key_fields: 1,
    measured: false,
    frozen: &[],
};

pub(crate) const RESERVED_FN_NAMES: ListSpec = ListSpec {
    file: "reserved_fn_names.txt",
    key_fields: 2,
    measured: false,
    frozen: &[],
};

pub(crate) const STRING_ERRORS: ListSpec = ListSpec {
    file: "string_errors.txt",
    key_fields: 2,
    measured: false,
    frozen: &[],
};

pub(crate) const SERDE_JSON_OUTSIDE_EDGES: ListSpec = ListSpec {
    file: "serde_json_outside_edges.txt",
    key_fields: 2,
    measured: false,
    frozen: &[
        "world_graph/modifying.rs WorldGraph::operate B5",
        "world_graph/modifying.rs WorldGraph::report B5",
        "world_graph/shape_store.rs Shape B5",
    ],
};

pub(crate) const JSON_VALUES_IN_SIGNATURES: ListSpec = ListSpec {
    file: "json_values_in_signatures.txt",
    key_fields: 2,
    measured: false,
    frozen: &[
        "world_graph/modifying.rs WorldGraph::report B5",
        "world_graph/shape_store.rs Shape B5",
    ],
};

pub(crate) const INLINE_TESTS: ListSpec = ListSpec {
    file: "inline_tests.txt",
    key_fields: 2,
    measured: false,
    frozen: &[],
};
