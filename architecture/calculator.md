# Calculator Core

The `src/calc/` module is the mathematical brain of eggsact. It accepts natural language expressions like *"what is the speed of light in miles per hour"* and returns precise, typed results. The module is organized into four files with a clear layered architecture.

## Module Overview

| File | Lines | Purpose |
|------|-------|---------|
| `context.rs` | 77 | Per-evaluation mutable state (`EvalContext`) for PRNG, memory registers, user variables, and function permissions |
| `normalize.rs` | ~2100 | Natural language pipeline: NL→math tokenization, 30-step `normalize()`, unit preprocessing, `split_at_operators()`, `run()`/`run_with_context()` orchestration |
| `evaluator.rs` | ~3740 | AST-based expression evaluator: tokenizer, recursive-descent parser, ~100 functions, big-integer arithmetic, helper algorithms |
| `units.rs` | ~2350 | Unit system: definitions, 500+ aliases, conversion factors, physical constants metadata, temperature conversion algorithm |

```
                 ┌──────────────────────────────────────────────┐
                 │              User Expression                  │
                 │   "thirty miles per hour in meters per sec"  │
                 └──────────────────┬───────────────────────────┘
                                    │
                 ┌──────────────────▼───────────────────────────┐
                 │           normalize.rs                        │
                 │  1. normalize() — 30-step NL pipeline         │
                 │  2. split_at_operators() — tokenization       │
                 │  3. preprocess_units() — unit detection/conversion │
                 │  4. add_same_unit_division_parens()            │
                 │  5. handle_convert_pattern() / handle_temp_pattern() │
                 └──────────────────┬───────────────────────────┘
                                    │
                 ┌──────────────────▼───────────────────────────┐
                 │          evaluator.rs                          │
                 │  1. tokenize() — hex/octal/binary/scientific  │
                 │  2. parse_bit_or → ... → parse_unary → parse_power → parse_primary │
                 │  3. evaluate_function() — ~100 functions       │
                 │  4. format_result() — int/float/str dispatch   │
                 └──────────────────┬───────────────────────────┘
                                    │
                 ┌──────────────────▼───────────────────────────┐
                 │          units.rs                              │
                 │  UNIT_ALIASES → UNIT_BASE → conversion         │
                 │  convert_temperature() — offset math           │
                 │  UnitValue — typed arithmetic with units       │
                 └──────────────────────────────────────────────┘
```

## Public API

The module re-exports from `lib.rs`:

```rust
// Full pipeline: NL normalization + math evaluation + unit detection
pub fn run(expr: &str) -> Result<(String, String), RunError>
pub fn run_with_context(expr: &str, ctx: &mut EvalContext) -> Result<(String, String), RunError>

// Direct math only (no NL normalization, no unit conversion)
pub fn evaluate(expr: &str) -> Result<(String, String), String>
pub fn evaluate_with_context(expr: &str, ctx: &mut EvalContext) -> Result<(String, String), String>

// Tokenizer used by normalize pipeline and MCP pre-checks
pub fn split_at_operators(expr: &str) -> Vec<String>

// MCP-safe mode control (legacy, idempotent one-shot)
pub fn set_mcp_mode()
pub fn is_mcp_mode() -> bool

// Type aliases
pub type EvaluateResult = (String, String); // (value, type)
pub type RunResult = (String, String);      // (value, type)
```

Return tuple: `(value_string, type_string)` where `type_string` is one of `"int"`, `"float"`, or `"str"`.

`RunError` distinguishes evaluation errors from internal processing errors:

```rust
pub enum RunError {
    Evaluation(String),  // parse error, division by zero, etc.
    Internal(String),    // normalization failure, input too long
}
```

## EvalContext (`context.rs`)

`EvalContext` holds per-evaluation mutable state, replacing global statics for state that varies between calls.

### Struct

```rust
#[derive(Clone)]
pub struct EvalContext {
    pub(crate) allow_random: bool,
    pub(crate) allow_side_effects: bool,
    pub(crate) prng_state: u64,
    pub(crate) gauss_spare: Option<f64>,
    pub(crate) memory_registers: HashMap<String, f64>,
    pub(crate) user_variables: HashMap<String, f64>,
}
```

| Field | Type | Purpose |
|-------|------|---------|
| `allow_random` | `bool` | Gates `random()`, `randint()`, `uniform()`, `randn()`, `gauss()`, `seed()` |
| `allow_side_effects` | `bool` | Gates `store()`, `recall()`, `mplus()`, `mminus()`, `mc()`, `mr()`, `setvar()`, `getvar()`, `delvar()`, `listvars()`, `clearvars()` |
| `prng_state` | `u64` | xorshift64 PRNG seed (default: `123456789`) |
| `gauss_spare` | `Option<f64>` | Box-Muller spare value for `randn()`/`gauss()` |
| `memory_registers` | `HashMap<String, f64>` | Calculator memory slots (`M`, `R0`–`R9`) |
| `user_variables` | `HashMap<String, f64>` | User-defined variables (`v0`, `v1`, …) capped at 1000 |

### Constructors and Builder Methods

| Method | Description |
|--------|-------------|
| `EvalContext::new()` | Default context: `allow_random=true`, `allow_side_effects=true`, `prng_state=123456789`, empty registers/variables |
| `EvalContext::mcp_mode()` | MCP-safe: `allow_random=false`, `allow_side_effects=false` |
| `.with_prng_state(state)` | Set PRNG seed for deterministic random sequences |
| `.with_memory_registers(regs)` | Initialize memory registers from a HashMap |
| `.with_user_variables(vars)` | Initialize user variables from a HashMap |

Builder methods use the builder pattern (consuming `self`), enabling chained construction:

```rust
let ctx = EvalContext::mcp_mode()
    .with_prng_state(42)
    .with_user_variables(my_vars);
```

### Context-Aware vs Legacy APIs

| API | State Source | Use When |
|-----|-------------|----------|
| `evaluate_with_context(expr, ctx)` | Caller-provided `EvalContext` | Need per-call state isolation (PRNG, memory, variables) |
| `run_with_context(expr, ctx)` | Caller-provided `EvalContext` | Full NL pipeline + per-call state isolation |
| `evaluate(expr)` | Legacy global statics | Simple cases, no state isolation needed |
| `run(expr)` | Legacy global statics | Simple NL pipeline, no state isolation needed |

**Critical**: `evaluate_with_context`/`run_with_context` operate directly on the caller's `ctx` — PRNG draws accumulate, memory mutations persist, user variables persist across calls. This is the correct API for multi-step calculations where state should accumulate.

### Thread-Local Bridge for MCP Dispatch

When `ToolRegistry::call_json_with_execution_context()` dispatches a tool, `ctx.eval_ctx` is **cloned** before the handler runs. The clone is set as a thread-local via `budget::with_eval_context()`, making it available to calculator-backed handlers (e.g., `math_eval`). Handler signature remains `fn(&Value) -> ToolResponse` — state isolation is achieved at the orchestration layer.

**Key invariant**: PRNG draws, memory mutations, and variable assignments inside the handler operate on the per-call clone and **do not propagate back** to the caller's `ExecutionContext`. Two calls with identical seeds produce the same first random value.

### What Remains Global

- `MCP_MODE`, `ALLOW_RANDOM`, `ALLOW_SIDE_EFFECTS` — `AtomicBool` flags set once at startup, one-shot idempotent. Read by `EvalContext` constructors.
- Legacy mutable globals `MEMORY_REGISTERS`, `USER_VARIABLES`, `PRNG_STATE`, `GAUSS_SPARE` — all `LazyLock<Mutex<...>>` for the legacy `evaluate()`/`run()` path only.

## Natural Language Pipeline (`normalize.rs`)

The `run()` function orchestrates the full pipeline:

```
┌─────────────┐    ┌──────────────────┐    ┌───────────────────┐    ┌──────────────┐    ┌───────────────┐
│  Input text  │───▶│  normalize()     │───▶│  split_at_        │───▶│  preprocess_ │───▶│  evaluate()   │
│  "thirty + 5"│    │  30-step NL→math │    │  operators()      │    │  units()     │    │  or convert() │
└─────────────┘    └──────────────────┘    └───────────────────┘    └──────────────┘    └───────────────┘
```

### The Complete 30-Step `normalize()` Pipeline

Every input passes through these steps in order. Steps marked with a reference ID correspond to specific bug fixes or parity requirements.

| Step | ID | Description | Example |
|------|----|-------------|---------|
| 1 | M16 | Replace unicode math operators with ASCII | `×` → `*`, `÷` → `/`, `−` → `-` |
| 2 | — | Lowercase entire expression | `THREE Plus FOUR` → `three plus four` |
| 3 | NZ-7 | Binary word validation — check before word replacement | `5 not 6` → error (ambiguous) |
| 4 | NZ-2 | Normalize lowercase temperature conversions | `100 c in f` → `100 C IN F` |
| 5 | M18 | Convert hyphens between number words to spaces | `twenty-one` → `twenty one` |
| 6 | — | Strip long filler phrases | `what's the value of` → `` |
| 7 | M19 | Handle "N to the M" power phrases | `2 to the 10` → `2**10` |
| 8 | NZ-8/M24 | Handle degrees→radian conversion (skip temperature) | `5 degrees` → `(5*pi/180)`, `100 degrees in fahrenheit` → `100` (temperature) |
| 9 | NZ-4 | Normalize spaced unit caret exponents | `5 m ^ 2` → `5 m2`, `/ m ^ 2` → `/m**2` |
| 10 | M23 | Handle "N thousand" scale words | `5 thousand` → `5000` |
| 11 | — | Convert `N%` to `(N/100)` | `50%` → `(50/100)` |
| 12 | — | Convert `N percent` to `(N/100)` | `50 percent` → `(50/100)` |
| 13 | — | Convert constant phrases to symbols | `gas constant` → `R`, `speed of light` → `c` |
| 14 | NZ-6 | Replace multi-word fraction numbers | `one half` → `0.5`, `two thirds` → `0.666...` |
| 15 | — | Convert number words to digits | `twenty one` → `20 1` |
| 16 | M20 | Handle "point" as decimal separator (before combining) | `three point one four` → `3.14` |
| 17 | — | Merge digits following decimal point | `3.1 4` → `3.14` |
| 18 | — | Combine consecutive number words | `twenty one` → `21`, `one hundred twenty two` → `122` |
| 19 | BUG-009 | Handle "kilometer per hour" forms | `60 kilometer per hour` → `60*km/h` |
| 20 | BUG-009 | Handle split-rate unit forms | `60 km / h` → `60*km/h` |
| 21 | BUG-009 | Bare compound unit forms | `60 km/h` → `60*km/h` |
| 22 | BUG-009 | Bare simple unit forms | `60 mph` → `60*mph` |
| 23 | M17 | Split compact function forms | `sin30` → `sin 30` |
| 24 | — | Convert operator words to symbols (length-descending) | `plus` → `+`, `raised to the power of` → `**` |
| 25 | — | Convert function names to standard forms (length-descending) | `square root` → `sqrt`, `natural log` → `log` |
| 26 | — | Fix "func * expr" patterns from "of"→"*" conversion | `half of 100` → `0.5 * 100` → `50` |
| 27 | NZ-9/M25 | Postfix unit power words | `m squared` → `m2`, `cm cubed` → `cm3` |
| 28 | NZ-10/M26 | Spelled unit conversions | `30 km/h in mph` → `convert(30*km/h,mph)` |
| 29 | — | Complex number patterns | `3+4i` → `(3+4j)` |
| 30 | M22 | Insert implicit multiplication | `5sin` → `5*sin`, `(2)(3)` → `(2)*(3)` |
| 31 | NZ-13/M21 | Factorial postfix | `5!` → `factorial(5)`, `5!!` → `factorial(factorial(5))` |

### Static Data Tables

#### `NUMBER_WORDS`

Maps English number words to their digit representations:

| Category | Words |
|----------|-------|
| Single digits | `zero`–`nine` → `"0"`–`"9"` |
| Teens | `ten`–`nineteen` → `"10"`–`"19"` |
| Tens | `twenty`–`ninety` → `"20"`, `"30"`, … `"90"` |
| Multipliers | `hundred` → `"100"`, `thousand` → `"1000"`, `million` → `"1000000"`, `billion` → `"1000000000"`, `trillion` → `"10^12"`, `quadrillion` → `"10^15"`, `quintillion` → `"10^18"` |
| Fractions | `half` → `"0.5"`, `quarter` → `"0.25"`, `thousandth` → `"0.001"`, `millionth` → `"0.000001"`, `billionth` → `"0.000000001"` |

#### `MULTI_WORD_NUMBERS`

Applied before individual number word replacement:

| Phrase | Value |
|--------|-------|
| `one half` | `0.5` |
| `one quarter` | `0.25` |
| `one third` | `0.3333333333333333` |
| `two thirds` | `0.6666666666666666` |
| `three quarters` | `0.75` |

#### `OPERATOR_CONVERSIONS`

Maps English operator phrases to symbols. Sorted by length descending during compilation to ensure longer patterns match first.

| Category | Phrases → Symbol |
|----------|-----------------|
| Arithmetic | `plus`, `positive` → `+`; `minus`, `negative` → `-`; `times`, `multiplied by` → `*`; `divided by`, `over`, `per`, `divide` → `/` |
| Power | `raised to the power of`, `raised to`, `to the power of` → `**` |
| Modulo | `mod`, `modulo`, `remainder` → `%` |
| Bitwise | `bitand`, `bit and` → `&`; `bitor`, `bit or`, `or` → `\|`; `bitxor`, `bit xor`, `xor` → `^`; `bitnot`, `bit not`, `not` → `~`; `left shift`, `shift left`, `lshift` → `<<`; `right shift`, `shift right`, `rshift` → `>>` |
| Conversion | `of` → `*`; `in`, `into` → `IN` (unit conversion keyword); `to`, `as` → `TO` (unit conversion keyword) |

#### `FUNCTION_MAPPINGS`

Maps English function names to standard forms, organized by category:

| Category | English Forms → Standard |
|----------|-------------------------|
| **Trigonometric** | `sine`→`sin`, `cosine`→`cos`, `tangent`→`tan`, `arc sine`/`arcsine`/`arcsin`/`inverse sine`→`asin`, `arc cosine`/`arccos`/`arccosine`→`acos`, `arc tangent`/`arctan`/`arctangent`→`atan`, `arc sin`→`asin`, `arc cos`→`acos`, `arc tan`→`atan` |
| **Hyperbolic** | `hyperbolic sine`→`sinh`, `hyperbolic cosine`→`cosh`, `hyperbolic tangent`→`tanh`, `arcsinh`→`asinh`, `arccosh`→`acosh`, `arctanh`→`atanh`, `inverse hyperbolic sine/cosine/tangent`→`asinh`/`acosh`/`atanh`, `hyperbolic arcsine/arccosine/arctangent`→`asinh`/`acosh`/`atanh` |
| **Logarithmic** | `logarithm`→`log`, `natural log`/`natural logarithm`/`ln`→`log`, `log base ten`/`log ten`→`log10`, `log two`/`log base two`→`log2` |
| **Power/Root** | `square root`→`sqrt`, `cube root`→`cbrt`, `root`→`sqrt` |
| **Rounding** | `absolute value`/`abs value`/`absolute`/`magnitude`→`abs`, `ceiling`→`ceil` |
| **Combinatorics** | `fact`→`factorial`, `nPr`→`nPr`, `nCr`→`nCr` |
| **Statistics** | `average`→`mean`, `stdev`→`std` |
| **Aggregate** | `gcd`→`gcd`, `lcm`→`lcm`, `perm`→`perm`, `comb`→`comb` |
| **Percentage** | `percent_of`→`percentof`, `as_percent`→`aspercent` |
| **Prime** | `is_prime`→`isprime`, `prime_factors`→`primefactors`, `next_prime`→`nextprime`, `prev_prime`→`prevprime` |
| **Complex** | `real`→`real`, `imag`→`imag`, `conj`/`conjugate`→`conj`, `phase`→`phase`, `polar`→`polar`, `rect`→`rect` |
| **Bitwise** | `bitand`→`bitand`, `bitor`→`bitor`, `bitxor`→`bitxor`, `bitnot`→`bitnot`, `bitlshift`→`bitlshift`, `bitrshift`→`bitrshift` |
| **Memory** | `store`→`store`, `recall`→`recall`, `Mplus`→`Mplus`, `Mminus`→`Mminus`, `MC`→`MC`, `MR`/`M`→`MR`, `setvar`→`setvar`, `getvar`→`getvar`, `delvar`→`delvar`, `listvars`→`listvars`, `clearvars`→`clearvars` |
| **Random** | `random`→`random`, `randint`→`randint`, `randrange`→`randrange`, `randn`→`randn`, `gauss`→`gauss`, `seed`→`seed` |
| **Self-mappings** | `convert`, `temp`, `floor`, `trunc`, `sign`, `degrees`, `radians`, `hypot`, `round`, `pow`, `atan2`, `log1p`, `expm1`, `uniform`, `cbrt`, `sqrt`, `log`, `log10`, `log2`, `abs`, `exp`, `ceil`, `clamp`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh` |

#### `CONSTANT_WORDS`

Maps English constant phrases to canonical symbol names:

| Category | Phrases → Symbol |
|----------|-----------------|
| Avogadro | `avogadro number`, `avogadro`, `avogadros` → `na` |
| Gas constant | `gas constant`, `molar gas constant`, `ideal gas constant` → `r` |
| Planck | `planck constant`, `planck` → `planckconstant` |
| Boltzmann | `boltzmann constant`, `boltzmann` → `k` |
| Speed of light | `speed of light`, `speed of light in vacuum`, `c zero` → `c` |
| Elementary charge | `elementary charge`, `e charge` → `elementarycharge` |
| Faraday | `faraday constant`, `faraday` → `f` |
| Atomic mass | `atomic mass`, `atomic mass unit`, `amu` → `u` |
| Permittivity | `vacuum permittivity`, `permittivity of free space` → `epsilon0` |
| Permeability | `vacuum permeability`, `permeability of free space`, `magnetic constant` → `mu0` |
| Gravity | `standard gravity`, `gravity`, `earth gravity` → `standardgravity` |
| Gravitational | `gravitational constant`, `newton constant`, `big g` → `G` |
| Particles | `electron mass`→`me`, `proton mass`→`mp`, `neutron mass`→`mn`, `classical electron radius`/`electron radius`→`re` |
| Electromagnetic | `fine structure constant`/`sommerfeld`→`alpha`, `rydberg constant`→`rydberg` |
| Radiation | `stefan boltzmann`/`stefan-boltzmann constant`→`stefan`, `wien constant`/`wien displacement`→`wien` |

#### `STRIPPED_PHRASES`

Filler phrases removed before evaluation:

| Long (stripped before operator conversion) | Short (stripped after) |
|-------------------------------------------|----------------------|
| `what's`, `what is`, `calculate`, `compute`, `tell me`, `give me`, `can you`, `could you`, `would you`, `i want to know`, `i'd like to know`, `what's the value of`, `what's the result of`, `what is the value of`, `what is the result of`, `the value of`, `the result of`, `the answer is` | `a`, `?`, `the `, `please `, `hey `, `hi `, `and ` |

### Compiled Regexes

All regexes are compiled via `LazyLock<Regex>` for zero-cost-after-initialization:

| Regex | Purpose |
|-------|---------|
| `PCT_SYMBOL_RE` | Match `N%` (not followed by digit) → `(N/100)` |
| `PERCENT_RE` | Match `N percent` → `(N/100)` |
| `COMPLEX_RE` | Match `N±Mi` complex number patterns → `(N±Mj)` |
| `TEMP_CONVERSION_RE` | Match `N unit in/to/as unit` temperature conversions |
| `BINARY_WORD_RE` | Check ambiguous binary word usage (`5 not 6`) |
| `TO_THE_POWER_RE` | Match `N to the M` → `N**M` |
| `DEGREES_NON_TEMP_RE` | Match `N degrees <non-temp-unit>` → angle conversion |
| `DEGREES_RE` | Match `N degrees` → `(N*pi/180)` |
| `POINT_RE` | Match `point` as decimal separator → `.` |
| `MERGE_DECIMAL_RE` | Merge `3.1 4` → `3.14` iteratively |
| `IMPLICIT_MUL_FUNC_RE` | Insert `*` between digit/`)` and function name |
| `IMPLICIT_MUL_PAREN_RE` | Insert `*` between `)` and digit/`(` |
| `IMPLICIT_MUL_DIGIT_PAREN_RE` | Insert `*` between digit and `(` |
| `FACTORIAL_RE` | Match `N!` or `func()!` → `factorial(N)` |
| `HYPHEN_RE` | Convert `twenty-one` → `twenty one` |
| `NUMBER_WORD_RE` | Match number words for replacement |
| `UNIT_CARET_ATTACH_RE` | Match `5 m ^ 2` → `5 m2` |
| `UNIT_CARET_DENOM_RE` | Match `/ m ^ 2` → `/m**2` |
| `UNIT_CARET_PAREN_RE` | Match `/(m) ^ 2` → `/(m)**2` |
| `UNIT_POWER_RE` | Match `m squared`/`m cubed` → `m2`/`m3` |
| `UNIT_SPELLED_RE` | Match `30 km/h in mph` → `convert(30*km/h,mph)` |
| `UNIT_COMPOUND_RE` | Match `60mi/h in m/s` → `convert(60*mi/h,m/s)` |
| `BARE_COMPOUND_UNIT_RE` | Insert `*` before compound units (`60 km/h` → `60*km/h`) |
| `SPLIT_UNIT_DIV_RE` | Match `60 km / h` → `60*km/h` |
| `PER_UNIT_RE` | Match `60 kilometer per hour` → `60*km/h` |
| `BARE_SIMPLE_UNIT_RE` | Insert `*` before simple spaced units (`60 mph` → `60*mph`) |
| `UNIT_INLINE_RE` | Scan for `<num>*<unit>` segments in joined post-split string |
| `SAME_UNIT_DIV_RE` | Wrap denominator for unit-on-division-right patterns |
| `CONVERT_SIMPLE_RE` | Match `convert(N*unit,target)` |
| `CONVERT_BARE_RE` | Match `convert(N,target)` |
| `TEMP_HANDLE_RE` | Match `temp(value,from,to)` |
| `COMPACT_FUNC_RE` | Split `sin30` → `sin 30` |
| `TEMP_DEG_CONV_PATTERNS` | Per-unit `N degrees in/to <temp_unit>` patterns (12 entries) |
| `TEMP_DEG_UNIT_PATTERNS` | Per-unit `N degrees <temp_unit>` patterns (12 entries) |
| `DIGIT_SCALE_PATTERNS` | `N thousand/million/…` scale patterns (7 entries) |
| `CONSTANT_PATTERNS` | Per-phrase constant replacement patterns (sorted by length descending) |
| `MULTI_WORD_PATTERNS` | Per-phrase multi-word fraction patterns |
| `STRIPPED_LONG_PATTERNS` | Per-phrase long filler patterns (>10 chars) |
| `STRIPPED_SHORT_PATTERNS` | Per-phrase short filler patterns (≤10 chars) |
| `OPERATOR_PATTERNS` | Per-word operator conversion patterns (sorted by length descending) |
| `FUNC_NAME_PATTERNS` | Per-word function name conversion patterns (sorted by length descending) |
| `FUNC_FIX_PATTERNS` | Per-function `func * expr` fix patterns |

### `split_at_operators()`

Tokenizes a normalized expression into `Vec<String>` by splitting on operators (`+`, `-`, `*`, `/`, `%`, `^`, `**`) at paren depth 0. Post-split cleanup handles edge cases:

- **Number-minus sequences**: `4-5-3` → `["4", "-", "5", "-", "3"]`
- **Double minus**: `4--5` → `["4", "-", "-5"]`
- **Trailing minus**: `5-(3)` → `["5", "-", "(3)"]`
- **Space-separated numbers**: `3 100 20 2` → `["3", "+", "100", "+", "20", "+", "2"]`
- **Scientific notation**: preserves `e+` / `e-` in `1.5e-3`

### `preprocess_units()`

After operator splitting, unit-bearing tokens like `"60*mph"` may have been broken apart. `preprocess_units()`:

1. Joins tokens into a single string
2. Scans for the first `<num>*<unit>` segment via `UNIT_INLINE_RE`
3. Detects the target unit (first unit found, or `%` for percentages)
4. Converts all other units to the target unit via `get_conversion_factor()`
5. Re-tokenizes the rewritten string

Returns `(re_tokens, Option<target_unit>)`.

### `run()` and `run_with_context()` Pipeline

```
1.  normalize(expr)                              — 30-step NL→math
2.  handle_convert_pattern(normalized)            — detect convert() → early return
3.  handle_temp_pattern(normalized)               — detect temp() → early return
4.  split_at_operators(normalized)                — tokenize
5.  preprocess_units(&tokens)                     — unit detection/conversion
6.  tokens.join("")                               — rejoin
7.  add_same_unit_division_parens(&processed)     — fix precedence for "5*m/3*s"
8.  evaluate(&processed) or evaluate_with_context  — math evaluation
9.  Append detected unit to result                — "34.296 m/s"
```

## Expression Evaluator (`evaluator.rs`)

### Tokenizer

The tokenizer handles:

| Format | Example | Parsed As |
|--------|---------|-----------|
| Decimal integers | `42` | `Number(42.0)` |
| Decimal floats | `3.14` | `Number(3.14)` |
| Leading decimal | `.5` | `Number(0.5)` |
| Scientific notation | `1.5e-3`, `1E3`, `.5e+2` | `Number(0.0015)`, etc. |
| Hex literals | `0xFF`, `0xDEAD` | `Number(255)`, `Number(57005)` |
| Octal literals | `0o17`, `0o77` | `Number(15)`, `Number(63)` |
| Binary literals | `0b1010`, `0b11111111` | `Number(10)`, `Number(255)` |
| Underscores in digits | `1_000_000`, `0xFF_FF` | `Number(1000000)`, `Number(65535)` |
| Power operator | `**` (not `* *`) | `Token::Power` |
| Floor division | `//` | `Token::FloorDiv` |
| Bitwise shifts | `<<`, `>>` | `Token::LShift`, `Token::RShift` |
| Identifiers | `sin`, `pi`, `myvar` | `Token::Identifier(name)` |
| Parentheses / commas | `(`, `)`, `,` | Matching tokens |

### Parser Precedence (8 Levels)

The parser is a recursive-descent implementation of standard operator precedence. Each level is implemented as a pair of functions: one for global statics (legacy), one context-aware.

| Precedence | Level | Operators | Associativity | Parse Function |
|------------|-------|-----------|---------------|----------------|
| 0 (lowest) | Bitwise OR | `\|` | Left | `parse_bit_or()` |
| 1 | Bitwise XOR | `^` | Left | `parse_bit_xor()` |
| 2 | Bitwise AND | `&` | Left | `parse_bit_and()` |
| 3 | Shift | `<<`, `>>` | Left | `parse_shift()` |
| 4 | Additive | `+`, `-` | Left | `parse_additive()` |
| 5 | Multiplicative | `*`, `/`, `//`, `%` | Left | `parse_multiplicative()` |
| 6 | Power | `**` | **Right** | `parse_power()` |
| 7 | Unary | `-`, `+`, `~` | Right | `parse_unary()` |
| 8 (highest) | Primary | numbers, identifiers, `(expr)` | — | `parse_primary()` |

**Important**: `^` is XOR, **not** exponentiation. Use `**` for power. This matches Python's `^` = XOR behavior.

**Right-associative power**: `2 ** 3 ** 2` = `2 ** (3 ** 2)` = `2 ** 9` = `512`, not `(2**3)**2` = `64`.

### All Functions by Category

#### Trigonometric (radians)

| Function | Args | Description |
|----------|------|-------------|
| `sin(x)` | 1 | Sine |
| `cos(x)` | 1 | Cosine |
| `tan(x)` | 1 | Tangent |
| `asin(x)` | 1 | Arc sine (inverse) |
| `acos(x)` | 1 | Arc cosine (inverse) |
| `atan(x)` | 1 | Arc tangent (inverse) |
| `atan2(y, x)` | 2 | Two-argument arc tangent |

#### Hyperbolic

| Function | Args | Description |
|----------|------|-------------|
| `sinh(x)` | 1 | Hyperbolic sine |
| `cosh(x)` | 1 | Hyperbolic cosine |
| `tanh(x)` | 1 | Hyperbolic tangent |
| `asinh(x)` | 1 | Inverse hyperbolic sine |
| `acosh(x)` | 1 | Inverse hyperbolic cosine |
| `atanh(x)` | 1 | Inverse hyperbolic tangent |

#### Logarithmic / Exponential

| Function | Args | Description |
|----------|------|-------------|
| `log(x)` / `ln(x)` | 1 | Natural logarithm |
| `log(x, base)` | 2 | Logarithm with arbitrary base (`ln(x)/ln(base)`) |
| `log10(x)` | 1 | Base-10 logarithm |
| `log2(x)` | 1 | Base-2 logarithm |
| `log1p(x)` | 1 | `ln(1 + x)`, accurate for small x |
| `exp(x)` | 1 | `e^x` (overflows at ~710) |
| `expm1(x)` | 1 | `e^x - 1`, accurate for small x |

#### Power / Root

| Function | Args | Description |
|----------|------|-------------|
| `sqrt(x)` | 1 | Square root (negative → error) |
| `cbrt(x)` | 1 | Cube root (handles negative) |
| `pow(a, b)` / `power(a, b)` | 2 | `a^b` (validates negative base, exponent range) |

#### Rounding / Absolute

| Function | Args | Description |
|----------|------|-------------|
| `abs(x)` | 1 | Absolute value |
| `floor(x)` | 1 | Floor |
| `ceil(x)` | 1 | Ceiling |
| `round(x)` | 1 | Banker's rounding (round half to even) |
| `round(x, n)` | 2 | Round to n decimal places |
| `trunc(x)` | 1 | Truncate toward zero |
| `sign(x)` | 1 | Signum: -1, 0, or 1 |

#### Angle Conversion

| Function | Args | Description |
|----------|------|-------------|
| `degrees(x)` | 1 | Radians → degrees |
| `radians(x)` | 1 | Degrees → radians |

#### Hypotenuse

| Function | Args | Description |
|----------|------|-------------|
| `hypot(x, ...)` | variadic | `sqrt(x² + y² + ...)` using scale-based algorithm to avoid overflow |

#### Factorial / Combinatorics

| Function | Args | Description |
|----------|------|-------------|
| `factorial(n)` / `fact(n)` | 1 | n! (big-integer, max n=1000, returns `__int_result__`) |
| `perm(n)` | 1 | n! (same as factorial, alias) |
| `perm(n, r)` / `nPr(n, r)` | 2 | Permutations P(n,r) (big-integer) |
| `comb(n, r)` / `nCr(n, r)` | 2 | Combinations C(n,r) (big-integer) |

#### GCD / LCM (variadic)

| Function | Args | Description |
|----------|------|-------------|
| `gcd(a, b, ...)` | variadic | Greatest common divisor (integer args required) |
| `lcm(a, b, ...)` | variadic | Least common multiple (integer args required) |

#### Aggregate (variadic)

| Function | Args | Description |
|----------|------|-------------|
| `sum(a, b, ...)` | variadic | Sum (empty → 0) |
| `max(a, b, ...)` | variadic | Maximum |
| `min(a, b, ...)` | variadic | Minimum |
| `mean(a, b, ...)` / `average` | variadic | Arithmetic mean |
| `median(a, b, ...)` | variadic | Median (odd → middle; even → average of two middle) |
| `mode(a, b, ...)` | variadic | Mode (first-most-frequent, handles `-0.0` normalization) |
| `product(a, b, ...)` | variadic | Product |

#### Statistics (variadic, ≥2 args required)

| Function | Args | Description |
|----------|------|-------------|
| `std(a, b, ...)` / `stddev` | ≥2 | Population standard deviation (÷ N) |
| `std_sample(a, b, ...)` / `stds` | ≥2 | Sample standard deviation (÷ N-1) |
| `variance(a, b, ...)` / `var` | ≥2 | Population variance (÷ N) |
| `variance_sample(a, b, ...)` / `vars` / `var_sample` | ≥2 | Sample variance (÷ N-1) |

#### Percentage

| Function | Args | Description |
|----------|------|-------------|
| `percentof(pct, value)` | 2 | `pct / 100 * value` |
| `aspercent(part, whole)` | 2 | `part / whole * 100` |

#### Clamp

| Function | Args | Description |
|----------|------|-------------|
| `clamp(x, lo, hi)` | 3 | `max(lo, min(hi, x))` (errors if lo > hi) |

#### Bitwise Functions

| Function | Args | Description |
|----------|------|-------------|
| `bitand(a, b)` | 2 | `a & b` (integer args) |
| `bitor(a, b)` | 2 | `a \| b` (integer args) |
| `bitxor(a, b)` | 2 | `a ^ b` (integer args) |
| `bitnot(a)` | 1 | `~a` (integer arg) |
| `bitlshift(a, n)` | 2 | `a << n` (checked shift) |
| `bitrshift(a, n)` | 2 | `a >> n` (checked shift) |

#### Base Conversion (returns `__string_result__`)

| Function | Args | Description |
|----------|------|-------------|
| `bin(n)` | 1 | Binary string (`0b1010`) |
| `hex(n)` | 1 | Hex string (`0xff`) |
| `oct(n)` | 1 | Octal string (`0o12`) |

#### Prime Number Functions

| Function | Args | Description |
|----------|------|-------------|
| `isprime(n)` / `is_prime` | 1 | Trial division up to √n, max n=10¹² → returns 0 or 1 |
| `nextprime(n)` / `next_prime` | 1 | Next prime after n (search limit: 10,000 candidates) |
| `prevprime(n)` / `prev_prime` | 1 | Previous prime before n (errors if n≤2) |
| `primefactors(n)` / `prime_factors` | 1 | Factorization string (`"2^2 × 3"`), max n=10¹² |

#### Complex Number Functions (f64 simplification)

| Function | Args | Description |
|----------|------|-------------|
| `real(z)` | 1 | Returns z (real part) |
| `imag(z)` | 1 | Returns 0.0 (imaginary part, f64-only) |
| `conj(z)` / `conjugate` | 1 | Returns z (conjugate, f64-only) |
| `phase(z)` / `arg` / `argument` | 1 | Returns 0 or π |
| `polar(z)` | 1 | Returns `(r, phi)` as `__string_result__` |
| `polar(r, phi)` | 2 | Pass-through as `__string_result__` |
| `rect(r, phi)` | 2 | Returns `(r*cos(φ), r*sin(φ))` as `__string_result__` |

#### Random Functions

| Function | Args | Description |
|----------|------|-------------|
| `random()` / `rand()` | 0 | Uniform [0, 1) via xorshift64 |
| `randint(a, b)` | 2 | Uniform integer in [a, b] |
| `randrange(n)` | 1 | Uniform integer in [0, n) |
| `randrange(a, b)` | 2 | Uniform integer in [a, b) |
| `uniform(a, b)` | 2 | Uniform float in [a, b) |
| `randn()` | 0 | Standard normal via Box-Muller |
| `gauss(mu, sigma)` / `normal` | 2 | Normal distribution μ+σ·N(0,1) |
| `seed(s)` | 0 or 1 | Set PRNG seed (0 → default seed `123456789`) |

#### Memory / Variable Functions

| Function | Args | Description |
|----------|------|-------------|
| `store(val)` | 1 | Store to memory register `M` |
| `store(val, n)` | 2 | Store to register `Rn` |
| `recall()` | 0 | Recall from register `M` |
| `recall(n)` | 1 | Recall from register `Rn` |
| `mplus(val)` / `m+` / `madd` | 1 | Add to register `M` |
| `mminus(val)` / `m-` / `msub` | 1 | Subtract from register `M` |
| `mc()` / `mclear` | 0 | Clear all memory registers |
| `mr()` / `mrecall` | 0 | Recall from register `M` (alias) |
| `setvar(val, id)` | 2 | Store value in variable `vid` |
| `getvar(id)` | 1 | Get variable (default 0) |
| `getvar(id, default)` | 2 | Get variable with default |
| `delvar(id)` | 1 | Delete variable |
| `listvars()` | 0 | List all variables as `__string_result__` |
| `clearvars()` | 0 | Delete all variables |

#### Unit Conversion (handled by `normalize.rs`, not evaluator)

| Function | Description |
|----------|-------------|
| `convert(value*unit, target)` | Unit conversion (intercepted by `run()`) |
| `temp(value, from, to)` | Temperature conversion (intercepted by `run()`) |

### Sentinel Protocol

The evaluator uses a sentinel-protocol pattern for returning non-numeric values through the `f64`-based function dispatch:

| Sentinel | Meaning | Example |
|----------|---------|---------|
| `__string_result__<value>` | String result (type `"str"`) | `bin(10)` → `"0b1010"` |
| `__int_result__<value>` | Large integer (type `"int"`) | `factorial(200)` → `"788..."` (375 digits) |

Both are implemented via `Err(EvaluationError::InvalidOperation(format!("__sentinel__{}", value)))`. The `evaluate()` function catches these and extracts the value:

```rust
Err(EvaluationError::InvalidOperation(ref s)) if s.starts_with("__string_result__") => {
    let value = s.trim_start_matches("__string_result__").to_string();
    Ok((value, "str".to_string()))
}
Err(EvaluationError::InvalidOperation(ref s)) if s.starts_with("__int_result__") => {
    let value = s.trim_start_matches("__int_result__").to_string();
    Ok((value, "int".to_string()))
}
```

### Big-Integer Arithmetic

`factorial()`, `perm()`, and `comb()` use base-1e9 little-endian big-integer arithmetic to avoid f64 rounding for large results.

**Representation**: `Vec<u64>` where each element is a digit in [0, 999,999,999], least-significant first.

| Function | Algorithm |
|----------|-----------|
| `multiply_in_place(limbs, n)` | Multiply each limb by `n` with carry, using `u128` intermediates |
| `divide_in_place(limbs, d)` | Divide each limb by `d` with remainder (exact division guaranteed by caller) |
| `limbs_to_string(limbs)` | Convert to decimal string |
| `bigint_or_float(limbs)` | Return as `f64` if fits in 2⁵³ mantissa, else `__int_result__` |

For `perm(n, r)`: multiply `n × (n-1) × ... × (n-r+1)`.

For `comb(n, r)`: interleave multiply-by-`(n-i)` with divide-by-`(i+1)` at each step so the partial result never exceeds the final value.

For `factorial(n)`: multiply `1 × 2 × ... × n`.

### Helper Algorithms

#### Banker's Rounding (`banker_round`)

Matches Python's `round()` — round half to even:

```rust
fn banker_round(x: f64) -> f64 {
    let floor = x.floor();
    let frac = x - floor;
    if (frac - 0.5).abs() < f64::EPSILON {
        if (floor as i64) % 2 == 0 { floor } else { floor + 1.0 }
    } else {
        x.round()
    }
}
```

#### PRNG (`xorshift64`)

Deterministic PRNG for `random()`, `randint()`, `randn()`, etc.:

```rust
fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
```

`random()` = `xorshift64() / u64::MAX`.

`randn()` uses Box-Muller transform with spare value caching (each call to `xorshift64` produces two normal variates; one is cached).

#### Prime Detection (`is_prime`)

Trial division with 6k±1 optimization:

```rust
fn is_prime(n: i64) -> bool {
    if n < 2 { return false; }
    if n < 4 { return true; }
    if n % 2 == 0 || n % 3 == 0 { return false; }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 { return false; }
        i += 6;
    }
    true
}
```

### Safety Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_NESTING_DEPTH` | 100 | Maximum parenthesis nesting depth |
| `MAX_EXPONENT` | 10,000.0 | Maximum `**` exponent magnitude |
| `MAX_RESULT_VALUE` | 1e308 | Maximum result magnitude (near f64 limit) |
| `MAX_SHIFT_COUNT` | 50,000 | Maximum bit shift count for `<<`/`>>` |
| `MAX_INPUT_LENGTH` | 10,000 | Maximum input expression length in characters |
| `MAX_FACTORIAL` | 1,000 | Maximum n for factorial (big-integer) |
| `MAX_PRIME` | 10¹² | Maximum n for isprime/nextprime/prevprime |
| `MAX_PERM_COMB` | 10,000 | Maximum n or r for perm/comb |
| `MAX_USER_VARIABLES` | 1,000 | Maximum user variables (evicts oldest on overflow) |
| `MAX_TEXT_LENGTH` (normalize) | 10,000 | Maximum normalized input length |

### MCP-Safe Mode

`set_mcp_mode()` (idempotent) disables:
- **Random functions**: `random`, `randint`, `randrange`, `uniform`, `randn`, `gauss`, `seed`
- **Side-effect functions**: `store`, `recall`, `mplus`, `mminus`, `mc`, `mr`, `setvar`, `getvar`, `delvar`, `listvars`, `clearvars`

Uses `AtomicBool` flags (`ALLOW_RANDOM`, `ALLOW_SIDE_EFFECTS`) checked at dispatch time. The context-aware API (`evaluate_with_context`) reads from `EvalContext` fields instead of the global flags.

## Units (`units.rs`)

### Complete Unit Category Table

#### Length (base: m)

| Unit | Aliases | to_base (m) |
|------|---------|-------------|
| `m` | meter, meters, metre, metres | 1.0 |
| `km` | kilometer, kilometers, kilometre, kilometres | 1000.0 |
| `cm` | centimeter, centimeters, centimetre, centimetres | 0.01 |
| `mm` | millimeter, millimeters, millimetre, millimetres | 0.001 |
| `um` | μm, micrometer, micrometers | 1e-6 |
| `nm` | nanometer, nanometers | 1e-9 |
| `pm` | picometer, picometers | 1e-12 |
| `inch` | in, inches | 0.0254 |
| `ft` | foot, feet | 0.3048 |
| `yd` | yard, yards | 0.9144 |
| `mi` | mile, miles | 1609.344 |
| `ly` | lightyear, lightyears | 9.4607304725808e15 |
| `au` | astronomicalunit, astronomicalunits | 1.49597870700e11 |
| `pc` | parsec, parsecs | 3.085677581491367e16 |
| `angstrom` | angstroms | 1e-10 |
| `fermi` | — | 1e-15 |
| `nmi` | nauticalmile, nauticalmiles | 1852.0 |
| `furlong` | furlongs | 201.168 |
| `chain` | chains | 20.1168 |
| `rod` | rd, rods | 5.0292 |
| `fathom` | fathoms | 1.8288 |
| `smoot` | smoots | 1.7018 |

#### Time (base: s)

| Unit | Aliases | to_base (s) |
|------|---------|-------------|
| `s` | second, seconds, sec, secs | 1.0 |
| `ms` | millisecond, milliseconds | 0.001 |
| `us` | μs, microsecond, microseconds | 1e-6 |
| `ns` | nanosecond, nanoseconds | 1e-9 |
| `ps` | picosecond, picoseconds | 1e-12 |
| `min` | minute, minutes | 60.0 |
| `h` | hr, hour, hours | 3600.0 |
| `d` | day, days | 86400.0 |
| `wk` | week, weeks | 604800.0 |
| `yr` | year, years | 31536000.0 |
| `fortnight` | fortnights | 1209600.0 |
| `decade` | decades | 315360000.0 |
| `century` | centuries | 3153600000.0 |
| `millennium` | millennia | 31536000000.0 |

#### Mass (base: kg)

| Unit | Aliases | to_base (kg) |
|------|---------|-------------|
| `kg` | kilogram, kilograms | 1.0 |
| `g` | gram, grams | 0.001 |
| `mg` | milligram, milligrams | 1e-6 |
| `ug` | μg, microgram, micrograms | 1e-9 |
| `ng` | nanogram, nanograms | 1e-12 |
| `lb` | lbs, pound, pounds | 0.45359237 |
| `oz` | ounce, ounces | 0.028349523125 |
| `ton` | tons (US short ton) | 907.18474 |
| `stone` | stones, st | 6.35029318 |
| `tonne` | tonnes (metric) | 1000.0 |
| `long_ton` | imperial_ton | 1016.0469 |
| `slug` | slugs | 14.593903 |
| `ct` | carat, carats | 0.0002 |
| `gr` | grain, grains | 6.479891e-5 |
| `dr` | dram, drams | 0.0017718452 |

#### Volume (base: L)

| Unit | Aliases | to_base (L) |
|------|---------|-------------|
| `L` | l, liter, liters, litre, litres | 1.0 |
| `mL` | milliliter, milliliters, millilitre, millilitres | 0.001 |
| `uL` | μL, microliter, microliters | 1e-6 |
| `gal` | gallon, gallons | 3.785411784 |
| `qt` | quart, quarts | 0.946352946 |
| `pt` | pint, pints | 0.473176473 |
| `cup` | cups | 0.2365882365 |
| `floz` | fl oz, fluidounce, fluidounces | 0.02957352954 |
| `tbsp` | tablespoon, tablespoons | 0.01478676477 |
| `tsp` | teaspoon, teaspoons | 0.00492892159 |
| `m3` | m^3, cubicmeter, cubicmeters | 1000.0 |
| `cm3` | cm^3, cc, cubiccentimeter, cubiccentimeters | 0.001 |
| `ft3` | ft^3, cubicfoot, cubicfeet | 28.316846592 |
| `in3` | in^3, cubicinch, cubicinches | 0.016387064 |
| `yd3` | yd^3, cubicyard, cubicyards | 764.554857984 |
| `mm3` | mm^3, cubicmillimeter, cubicmillimeters | 1e-6 |
| `km3` | km^3, cubickilometer, cubickilometers | 1e12 |
| `mi3` | mi^3, cubicmile, cubicmiles | 4.168181825e12 |

#### Data (base: B)

| Unit | Aliases | to_base (B) |
|------|---------|-------------|
| `B` | byte, bytes | 1.0 |
| `bit` | bits, b | 0.125 |
| `KB` | kilobyte, kilobytes | 1024.0 |
| `MB` | megabyte, megabytes | 1048576.0 |
| `GB` | gigabyte, gigabytes | 1073741824.0 |
| `TB` | terabyte, terabytes | 1099511627776.0 |
| `PB` | petabyte, petabytes | 1125899906842624.0 |
| `EB` | exabyte, exabytes | 1152921504606846976.0 |
| `ZB` | zettabyte, zettabytes | 1.1805916207174113e21 |
| `YB` | yottabyte, yottabytes | 1.2089258196146292e24 |

#### Data Transfer Rate (base: bps)

| Unit | Aliases | to_base (bps) |
|------|---------|-------------|
| `bps` | bit/s, bits/s | 1.0 |
| `Kbps` | kilobps, kilobit/s, kilobits/s | 1000.0 |
| `Mbps` | megabps, megabit/s, megabits/s | 1000000.0 |
| `Gbps` | gigabps, gigabit/s, gigabits/s | 1000000000.0 |

#### Pressure (base: Pa)

| Unit | Aliases | to_base (Pa) |
|------|---------|-------------|
| `Pa` | pascal, pascals | 1.0 |
| `kPa` | kilopascal, kilopascals | 1000.0 |
| `MPa` | megapascal, megapascals | 1e6 |
| `GPa` | gigapascal, gigapascals | 1e9 |
| `bar` | bars | 100000.0 |
| `mbar` | millibar | 100.0 |
| `atm` | atmosphere, atmospheres | 101325.0 |
| `psi` | psia | 6894.757293168 |
| `mmHg` | — | 133.32236842105 |
| `torr` | — | 133.32236842105 |
| `inHg` | — | 3386.389 |
| `mmH2O` | — | 9.80665 |
| `inH2O` | — | 249.08891 |

#### Energy (base: J)

| Unit | Aliases | to_base (J) |
|------|---------|-------------|
| `J` | joule, joules | 1.0 |
| `kJ` | kilojoule, kilojoules | 1000.0 |
| `MJ` | megajoule, megajoules | 1e6 |
| `GJ` | gigajoule, gigajoules | 1e9 |
| `cal` | calorie, calories | 4.184 |
| `kcal` | kilocalorie, kilocalories | 4184.0 |
| `Wh` | watt-hour, watt-hours | 3600.0 |
| `kWh` | kilowatt-hour, kilowatt-hours | 3.6e6 |
| `BTU` | btu | 1055.05585262 |
| `eV` | ev, electronvolt, electronvolts | 1.602176634e-19 |

#### Power (base: W)

| Unit | Aliases | to_base (W) |
|------|---------|-------------|
| `W` | watt, watts | 1.0 |
| `kW` | kilowatt, kilowatts | 1000.0 |
| `MW` | megawatt, megawatts | 1e6 |
| `GW` | gigawatt, gigawatts | 1e9 |
| `mW` | milliwatt, milliwatts | 0.001 |
| `hp` | horsepower | 745.6998715822702 |

#### Force (base: N)

| Unit | Aliases | to_base (N) |
|------|---------|-------------|
| `N` | newton, newtons | 1.0 |
| `kN` | kilonewton | 1000.0 |
| `mN` | millinewton | 0.001 |
| `dyne` | dynes | 1e-5 |
| `lbf` | poundforce | 4.4482216152605 |

#### Voltage (base: V)

| Unit | Aliases | to_base (V) |
|------|---------|-------------|
| `V` | volt, volts | 1.0 |
| `kV` | kilovolt | 1000.0 |
| `mV` | millivolt | 0.001 |
| `μV` | uV, microvolt | 1e-6 |

#### Current (base: A)

| Unit | Aliases | to_base (A) |
|------|---------|-------------|
| `A` | amp, ampere, amperes | 1.0 |
| `mA` | milliamp, milliampere | 0.001 |
| `μA` | uA, microamp, microampere | 1e-6 |

#### Angle (base: rad)

| Unit | Aliases | to_base (rad) |
|------|---------|-------------|
| `rad` | radian, radians | 1.0 |
| `deg` | degree, degrees, ° | 0.017453292519943295 (π/180) |

#### Speed (base: m/s)

| Unit | Aliases | to_base (m/s) |
|------|---------|-------------|
| `m/s` | mps, meterpersecond, meterspersecond | 1.0 |
| `km/h` | kph, kmh, kilometerperhour, kilometersperhour | 1000.0/3600.0 |
| `mph` | mileperhour, milesperhour, mi/h | 0.44704 |
| `kn` | knot, knots, kt | 1852.0/3600.0 |
| `mach` | — | 340.29 |

#### Temperature (base: K, special conversion)

| Unit | Aliases |
|------|---------|
| `K` | kelvin, kelvins, °K, degk |
| `C` | celsius, centigrade, °C, degc |
| `F` | fahrenheit, °F, degf |
| `Ra` | rankine, °R, degr |

#### Area (base: m²)

| Unit | Aliases | to_base (m²) |
|------|---------|-------------|
| `m2` | m^2, m**2, sqm, squaremeter, squaremeters | 1.0 |
| `km2` | km^2, km**2, squarekilometer, squarekilometers | 1e6 |
| `cm2` | cm^2, cm**2, squarecentimeter, squarecentimeters | 1e-4 |
| `mm2` | mm^2, mm**2, squaremillimeter, squaremillimeters | 1e-6 |
| `ha` | hectare, hectares | 10000.0 |
| `acre` | acres | 4046.8564224 |
| `ft2` | ft^2, ft**2, sqft, squarefoot, squarefeet | 0.09290304 |
| `in2` | in^2, in**2, sqin, squareinch, squareinches | 0.00064516 |
| `mi2` | mi^2, mi**2, sqmi, squaremile, squaremiles | 2589988.110336 |
| `yd2` | yd^2, yd**2, sqyd, squareyard, squareyards | 0.83612736 |

#### Frequency (base: Hz)

| Unit | Aliases | to_base (Hz) |
|------|---------|-------------|
| `Hz` | hertz | 1.0 |
| `kHz` | kilohertz | 1000.0 |
| `MHz` | megahertz | 1e6 |
| `GHz` | gigahertz | 1e9 |
| `THz` | terahertz | 1e12 |

### `UNIT_ALIASES` Overview

`UNIT_ALIASES` is a `LazyLock<HashMap<&str, &str>>` containing 500+ entries mapping every recognized unit name (including plurals, abbreviations, case variations, Unicode variants like `μm`/`um`, and alternative spellings like `litre`/`liter`) to its canonical form in `UNIT_BASE`.

Key design decisions:
- **Case-insensitive fallback**: `is_unit()` tries exact match, then lowercase, uppercase, title case, and capital-case
- **Compound unit `**` forms**: `m**2`, `cm**2`, etc. are registered as aliases for `m2`, `cm2`
- **`b` vs `B`**: lowercase `b` aliases to `bit`, uppercase `B` to `byte` (BUG-207)
- **Temperature aliases**: `degf`→`F`, `degc`→`C`, `degk`→`K`, `degr`→`Ra`, plus Unicode `°F`, `°C`, etc.

### Temperature Conversion Algorithm

Temperature uses **offset-based** conversion through an intermediate Celsius value:

```
input → Celsius → target
```

| From → Celsius | To Celsius → Target |
|---------------|---------------------|
| C: `v` | C: `c` |
| F: `(v - 32) × 5/9` | F: `c × 9/5 + 32` |
| K: `v - 273.15` | K: `c + 273.15` |
| Ra: `(v - 491.67) × 5/9` | Ra: `(c + 273.15) × 9/5` |

`get_conversion_factor()` explicitly rejects temperature units with an error directing callers to `convert_temperature()`.

### `UnitValue` Operator Overloading

`UnitValue` implements `Add`, `Sub`, `Mul`, `Div` operators that propagate units through arithmetic:

| Operation | Unit Behavior |
|-----------|--------------|
| `km + km` | Result unit: `km` |
| `km + m` | Auto-converts m→km (same category) |
| `km * m` | Result unit: `km*m` |
| `km * km` | Result unit: `km**2` |
| `km / km` | Result unit: `None` (dimensionless) |
| `km / m` | Result unit: `km/m` |
| `km + None` | Result unit: `km` |

`Add`/`Sub` return `Result<Self, String>` (incompatible categories → error). `Mul`/`Div` are infallible.

### Physical Constants Metadata Table

`PHYSICAL_CONSTANTS` provides display metadata (symbol, display name) for the calculator's constants, used by `constant_lookup` MCP tool:

| Constant | Symbol | Display Name | Value |
|----------|--------|-------------|-------|
| `pi` | π | Pi | 3.141592653589793 |
| `e` | e | Euler's number | 2.718281828459045 |
| `tau` | τ | Tau | 6.283185307179586 |
| `na`/`avogadro` | N_A | Avogadro constant | 6.02214076e23 |
| `r`/`R`/`gasconstant` | R | Gas constant | 8.314462618 |
| `h`/`planck` | h | Planck constant | 6.62607015e-34 |
| `hbar`/`planckbar` | ℏ | Reduced Planck constant | 1.054571817e-34 |
| `k`/`boltzmann` | k_B | Boltzmann constant | 1.380649e-23 |
| `c`/`speedoflight` | c | Speed of light in vacuum | 299792458.0 |
| `G`/`gravitationalconstant` | G | Gravitational constant | 6.67430e-11 |
| `qe`/`elementarycharge` | e | Elementary charge | 1.602176634e-19 |
| `me`/`electronmass` | mₑ | Electron mass | 9.1093837015e-31 |
| `mp`/`protonmass` | mₚ | Proton mass | 1.67262192369e-27 |
| `mn`/`neutronmass` | mₙ | Neutron mass | 1.67493e-27 |
| `u`/`amu` | u | Atomic mass unit | 1.66053906660e-27 |
| `re`/`electronradius` | rₑ | Classical electron radius | 2.8179403262e-15 |
| `alpha`/`finestructure` | α | Fine-structure constant | 7.2973525693e-3 |
| `f`/`faraday` | F | Faraday constant | 96485.33212 |
| `epsilon0`/`vacuumpermittivity` | ε₀ | Vacuum permittivity | 8.8541878128e-12 |
| `mu0`/`vacuumpermeability` | μ₀ | Vacuum permeability | 1.25663706212e-6 |
| `standardgravity` | gₙ | Standard gravity | 9.80665 |
| `rydberg` | R∞ | Rydberg constant | 10973731.568160 |
| `stefan`/`stefanboltzmann` | σ | Stefan-Boltzmann constant | 5.670374419e-8 |
| `wien`/`wienconstant` | b | Wien displacement constant | 2.897771955e-3 |

### `get_conversion_factor()`

Returns the multiplicative factor to convert from one unit to another:

1. Resolve both units through `UNIT_ALIASES`
2. Look up `UNIT_BASE` for both
3. Validate same category (different → error)
4. Reject temperature (→ error with redirect to `convert_temperature()`)
5. Return `from_def.to_base / to_def.to_base`

### `is_unit()`

Multi-strategy unit lookup:

1. Exact match in `UNIT_ALIASES` → check `UNIT_BASE`
2. Direct match in `UNIT_BASE`
3. Case variations: lowercase, uppercase, title case, capital case
4. Each variation checked against both `UNIT_ALIASES` and `UNIT_BASE`

### `get_unit_info()`

Returns `(canonical_name, category)` for a unit, or `None` if unknown.

## Key Design Patterns

### Dual API Paths

Every function exists in two variants: legacy (global statics) and context-aware (per-call `EvalContext`). Legacy wrappers create a default `EvalContext` internally and delegate to the context-aware version. New code should use context-aware APIs.

### Sentinel Protocol

Functions that return non-numeric values (strings, large integers) use `Err(InvalidOperation("__sentinel__value"))` to pass through the `f64`-based dispatch. The `evaluate()`/`evaluate_with_context()` functions catch these and convert to `Ok((value, type))`.

### LazyLock for Zero-Cost Statics

All static data (regexes, lookup tables, compiled alternation strings) use `std::sync::LazyLock` for lazy initialization on first access. Once initialized, access is lock-free.

### `^` is XOR, `**` is Power

This matches Python's operator semantics. The NL pipeline converts English power phrases (`raised to the power of`, `to the power of`) to `**`. The `^` token is exclusively bitwise XOR.

### `g` Means Gram

In unit expressions, `g` resolves to `UNIT_ALIASES["g"]` = `"gram"` = 0.001 kg. To reference the standard gravity constant, use `gravity` or `standardgravity`. The evaluator checks `UNIT_ALIASES` before `CONSTANTS`, so `g` is always gram.

### Temperature Is Special

Temperature units use offset-based conversion (add/subtract constants), not multiplicative factors. `get_conversion_factor()` rejects temperature with a specific error. The `convert_temperature()` function handles the Celsius intermediate conversion. The NL pipeline has dedicated regexes (`TEMP_CONVERSION_RE`, `TEMP_DEG_CONV_PATTERNS`) to detect and canonicalize temperature conversion phrases before operator word replacement consumes the prepositions.
