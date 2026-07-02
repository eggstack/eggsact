/// Validation and formatting policy for tool calls.
///
/// Controls whether tool call validation and error messages use
/// Python-parity behavior (`EggcalcPython`) or strict JSON Schema
/// behavior (`StrictNative`).
///
/// # MCP Boundary
///
/// The MCP server uses `EggcalcPython` to preserve compatibility with
/// Python `eggcalc` clients. This includes Python-style type names in
/// error messages (`NoneType`, `int`, `float`, `str`, `list`, `dict`)
/// and bool-as-int coercion for numeric parameters.
///
/// # In-Process API
///
/// `ToolRegistry::default()` uses `StrictNative`, which produces
/// standard JSON Schema type names (`null`, `integer`, `number`,
/// `string`, `array`, `object`) and rejects ambiguous inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CompatibilityMode {
    /// Python-parity behavior for MCP migration and general clients.
    ///
    /// Preserves Python-style type names in error messages and allows
    /// bool-as-int coercion for numeric parameters at the MCP boundary.
    EggcalcPython,
    /// Strict native behavior for codegg and Rust consumers.
    ///
    /// Uses standard JSON Schema type names and rejects ambiguous or
    /// incorrectly typed inputs.
    #[default]
    StrictNative,
}
