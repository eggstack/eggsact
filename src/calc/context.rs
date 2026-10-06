use std::collections::{HashMap, VecDeque};

/// Bounded user-variable store used by `setvar`/`getvar`/`delvar`/`listvars`.
///
/// `values` holds the bindings; `order` records insertion order so capacity
/// eviction drops the *oldest* binding. Evicting by key order instead made the
/// survivor set unpredictable: a freshly created low key (`v1`) became the
/// minimum and was dropped on the very next insert.
#[derive(Clone)]
pub(crate) struct UserVarStore {
    values: HashMap<String, f64>,
    order: VecDeque<String>,
}

impl UserVarStore {
    pub(crate) fn new() -> Self {
        Self {
            values: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(crate) fn from_values(values: HashMap<String, f64>) -> Self {
        // A caller-supplied map has no recorded order; seed it from the sorted
        // key list so eviction stays deterministic.
        let mut keys: Vec<String> = values.keys().cloned().collect();
        keys.sort();
        Self {
            values,
            order: keys.into(),
        }
    }

    /// Insert or overwrite `key`, evicting the oldest bindings first so `len()`
    /// never exceeds `capacity`.
    pub(crate) fn insert(&mut self, key: String, value: f64, capacity: usize) {
        if let Some(slot) = self.values.get_mut(&key) {
            *slot = value;
            return;
        }
        while self.values.len() >= capacity {
            match self.order.pop_front() {
                Some(oldest) => {
                    self.values.remove(&oldest);
                }
                None => break,
            }
        }
        self.order.push_back(key.clone());
        self.values.insert(key, value);
    }

    pub(crate) fn get(&self, key: &str) -> Option<f64> {
        self.values.get(key).copied()
    }

    pub(crate) fn remove(&mut self, key: &str) {
        self.values.remove(key);
        self.order.retain(|k| k != key);
    }

    pub(crate) fn clear(&mut self) {
        self.values.clear();
        self.order.clear();
    }

    /// Render the bindings as `{v1: 1, v2: 2}`, sorted by key so the string is
    /// stable across processes (`HashMap` iteration order is randomized).
    pub(crate) fn to_display_string(&self) -> String {
        if self.values.is_empty() {
            return "{}".to_string();
        }
        let mut entries: Vec<(&String, &f64)> = self.values.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let rendered: Vec<String> = entries
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect();
        format!("{{{}}}", rendered.join(", "))
    }
}

/// Per-evaluation mutable state for context-aware calculator evaluation.
///
/// `EvalContext` provides explicit PRNG, Gaussian spare, memory-register,
/// user-variable, and function-permission state for `evaluate_with_context()`
/// and `run_with_context()`. Legacy `evaluate()` and `run()` remain
/// backward-compatible and use the legacy process-global compatibility state.
///
/// When used through `ToolRegistry::call_json_with_execution_context()`, the
/// context is currently cloned as a per-call seed/template for `math_eval`; state
/// mutations do not persist back into the caller's `ExecutionContext`.
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
    pub(crate) user_variables: UserVarStore,
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
            user_variables: UserVarStore::new(),
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
            user_variables: UserVarStore::new(),
        }
    }

    /// Set the PRNG state for deterministic random sequences.
    ///
    /// A `state` of `0` is remapped to `123456789` to avoid poisoning the
    /// xorshift64 PRNG (which would otherwise output all zeros). This matches
    /// the guard applied by `prng_seed_with` in the evaluator, so the two
    /// seeding paths behave consistently.
    pub fn with_prng_state(mut self, state: u64) -> Self {
        self.prng_state = if state == 0 { 123456789 } else { state };
        self
    }

    /// Set the memory registers.
    pub fn with_memory_registers(mut self, registers: HashMap<String, f64>) -> Self {
        self.memory_registers = registers;
        self
    }

    /// Set the user variables.
    pub fn with_user_variables(mut self, variables: HashMap<String, f64>) -> Self {
        self.user_variables = UserVarStore::from_values(variables);
        self
    }
}

impl Default for EvalContext {
    fn default() -> Self {
        Self::new()
    }
}
