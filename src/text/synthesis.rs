//! Higher-level text synthesis and analysis tools.
//!
//! In the Python reference, this module provides convenience wrappers
//! that combine lower-level `exact/` primitives into higher-level
//! operations. In the Rust implementation, these functions are
//! available directly from their canonical submodules.

pub mod detail {
    //! Mapping of Python `synthesis` functions to their Rust sources:
    //!
    //! | Python function | Rust source |
    //! |---|---|
    //! | `measure_text` | `measure` module |
    //! | `text_equal` | inline in `mcp::tools` |
    //! | `explain_diff` | `diff` module |
    //! | `inspect_text` | `primitives` + `confusables` |
    //! | `count_chars` | `measure::char_frequency` |
    //! | `text_replace_check` | `replace` module |
    //! | `text_window` | `position` module |
    //! | `line_range_extract` | `line_range` module |
    //! | `line_range_compare` | `line_range` module |
    //! | `list_compare` | inline in `mcp::tools` |
}
