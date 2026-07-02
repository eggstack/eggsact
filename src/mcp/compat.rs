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
/// error messages (`NoneType`, `int`, `float`, `str`, `list`, `dict`).
/// JSON booleans are always rejected for numeric parameters in both
/// modes because MCP model-generated booleans for number fields are
/// commonly mistakes.
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
    /// Preserves Python-style type names in error messages (e.g.,
    /// `NoneType` instead of `null`, `int` instead of `integer`).
    /// Bool values are rejected for numeric schema fields.
    EggcalcPython,
    /// Strict native behavior for codegg and Rust consumers.
    ///
    /// Uses standard JSON Schema type names and rejects ambiguous or
    /// incorrectly typed inputs. Bool values are rejected for numeric
    /// schema fields.
    #[default]
    StrictNative,
}
