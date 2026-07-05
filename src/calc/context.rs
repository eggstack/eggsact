use std::collections::HashMap;

/// Per-evaluation mutable state for the calculator evaluator.
///
/// Replaces the global statics (PRNG_STATE, GAUSS_SPARE, MEMORY_REGISTERS,
/// USER_VARIABLES) with explicit per-evaluation state, enabling deterministic
/// and isolated calculator calls.
#[derive(Clone)]
pub struct EvalContext {
    /// Whether random functions are allowed (rejects random/side-effect functions when false).
    pub(crate) allow_random: bool,
    /// Whether side-effect functions are allowed (rejects memory/variable functions when false).
    pub(crate) allow_side_effects: bool,
    /// xorshift64 PRNG state.
    pub(crate) prng_state: u64,
    /// Box-Muller spare for randn/gauss.
    pub(crate) gauss_spare: Option<f64>,
    /// Memory registers for store/recall/mplus/mminus/mc/mr.
    pub(crate) memory_registers: HashMap<String, f64>,
    /// User variables for setvar/getvar/delvar/listvars/clearvars.
    pub(crate) user_variables: HashMap<String, f64>,
}

impl EvalContext {
    /// Create a default context with all functions allowed.
    pub fn new() -> Self {
        Self {
            allow_random: true,
            allow_side_effects: true,
            prng_state: 123456789,
            gauss_spare: None,
            memory_registers: HashMap::new(),
            user_variables: HashMap::new(),
        }
    }

    /// Create an MCP-safe context with random and side-effect functions disabled.
    pub fn mcp_mode() -> Self {
        Self {
            allow_random: false,
            allow_side_effects: false,
            prng_state: 123456789,
            gauss_spare: None,
            memory_registers: HashMap::new(),
            user_variables: HashMap::new(),
        }
    }

    /// Set the PRNG state for deterministic random sequences.
    pub fn with_prng_state(mut self, state: u64) -> Self {
        self.prng_state = state;
        self
    }

    /// Set the memory registers.
    pub fn with_memory_registers(mut self, registers: HashMap<String, f64>) -> Self {
        self.memory_registers = registers;
        self
    }

    /// Set the user variables.
    pub fn with_user_variables(mut self, variables: HashMap<String, f64>) -> Self {
        self.user_variables = variables;
        self
    }
}

impl Default for EvalContext {
    fn default() -> Self {
        Self::new()
    }
}
