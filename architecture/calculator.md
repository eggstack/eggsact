# Calculator Core

The `src/calc/` module handles math expression parsing and evaluation.

## Modules

| Module | Purpose |
|--------|---------|
| `normalize.rs` | Natural language tokenization, number word parsing, unit preprocessing |
| `evaluator.rs` | AST-based expression evaluation, function dispatch |
| `units.rs` | Unit definitions, conversion factors, physical/mathematical constants |

## Public API (re-exported from lib.rs)

```rust
pub fn run(expr: &str) -> Result<(String, String), RunError>        // NL + math + units
pub fn evaluate(expr: &str) -> Result<(String, String), String>      // direct math only
pub fn split_at_operators(expr: &str) -> Vec<String>                 // tokenizer
```

## Natural Language Pipeline (`run()`)

1. **Preprocessing**: `preprocess_units()` handles spaced/compound units (`60 mph`, `km per hr`)
2. **Normalization**: `normalize()` tokenizes English text into math tokens
   - Number words: `five` → `5`, `twenty` → `20`
   - Operator words: `plus` → `+`, `times` → `*`
   - Function names: `square root of` → `sqrt(`
   - Percentages: `50 percent of` → `50 * 0.01 *`
   - Fillers removed: `what is`, `calculate`, etc.
3. **Evaluation**: `evaluator.rs` parses and evaluates the normalized expression

## Expression Evaluator

AST-based evaluator supporting:
- Arithmetic: `+`, `-`, `*`, `/`, `%`, `**` (power), `^` (XOR)
- Parentheses for grouping
- Functions: trig, log, abs, floor, ceil, round, factorial, etc.
- Constants: `pi`, `e`, `tau`, `phi`, `c`, `gravity`, `na`, etc.
- Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Complex numbers: `3+4j`
- Statistical: `sum`, `mean`, `median`, `std`, `variance`, `min`, `max`, `product`
- Number theory: `gcd`, `lcm`, `factorial`, `isprime`, `nextprime`, `prevprime`
- Combinatorics: `perm`, `comb`
- Complex math: `polar`, `rect`

### Big Integer Arithmetic

`factorial()`, `perm()`, and `comb()` use base-1e9 big-integer arithmetic.
Results within the 53-bit f64 mantissa return as float.
Larger values surface via `__int_result__` sentinel (MCP `type: "int"`).

## Units

### Supported Categories

| Category | Units |
|----------|-------|
| Length | m, km, cm, mm, in, ft, yd, mi, ly, au, pc |
| Mass | kg, g, mg, ug, ng, lb, oz, ton, stone |
| Time | s, ms, us, ns, min, h, d, wk, yr |
| Volume | L, mL, gal, qt, pt, cup, floz, tbsp, tsp |
| Temperature | C, F, K |
| Data | B, KB, MB, GB, TB |
| Pressure | Pa, kPa, MPa, GPa, bar, atm, psi |
| Energy | J, kJ, cal, kcal, Wh, kWh, BTU, eV |
| Power | W, kW, MW, GW, hp |
| Force | N, kN, dyne, lbf |
| Voltage | V, kV, mV |
| Current | A, mA |
| Angle | rad, deg |
| Speed | m/s, km/h, mph, kn, mach |
| Frequency | Hz, kHz, MHz, GHz, THz |

### Temperature Conversions

Temperature uses offset math, not multiplicative factors.
`TEMP_CONVERSION_RE` accepts zero-width whitespace between number and unit.
`resolve_unit_canon()` does case-insensitive alias lookup.

### Prefixed Units

Prefixed units (`kN`, `mV`, `mA`) are supported via SI prefix lookup.

### Constants

Mathematical: `pi`, `e`, `tau`, `phi`
Physical: `c`, `h`, `hbar`, `k`, `G`, `na`, `R`, `qe`, `me`, `mp`, `mn`, `epsilon0`, `mu0`, `gravity`, `atm`

**Important**: `g` means gram in unit expressions. Use `gravity` or `standardgravity` for standard gravity.
