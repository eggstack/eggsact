# Math Features

eggsact's calculator supports natural language input, direct math expressions, unit conversions, and physical/mathematical constants.

See also: [Library API](library-api.md), [Calculator Core](../architecture/calculator.md)

## Operations

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `3 + 4` = 7 |
| `-` | Subtraction | `10 - 3` = 7 |
| `*` | Multiplication | `3 * 4` = 12 |
| `/` | Division | `10 / 3` = 3.333... |
| `%` | Modulo | `10 % 3` = 1 |
| `**` | Power | `2 ** 10` = 1024 |
| `^` | Bitwise XOR (not exponentiation) | `5 ^ 3` = 6 |
| `(` `)` | Grouping | `(10 + 2) / 4` = 3 |

**Right-associative power**: `2 ** 3 ** 2` = `2 ** (3 ** 2)` = 512, not 64.

## Functions

### Trigonometric (radians)

`sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2(y, x)`

### Hyperbolic

`sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`

### Logarithmic / Exponential

`log(x)` / `ln(x)`, `log(x, base)`, `log10(x)`, `log2(x)`, `log1p(x)`, `exp(x)`, `expm1(x)`

### Power / Root

`sqrt(x)`, `cbrt(x)`, `pow(a, b)` / `power(a, b)`

### Rounding / Absolute

`abs(x)`, `floor(x)`, `ceil(x)`, `round(x)`, `round(x, n)`, `trunc(x)`, `sign(x)`

### Angle Conversion

`degrees(x)`, `radians(x)`

### Factorial / Combinatorics

`factorial(n)` / `fact(n)`, `perm(n, r)` / `nPr(n, r)`, `comb(n, r)` / `nCr(n, r)`

### GCD / LCM

`gcd(a, b, ...)`, `lcm(a, b, ...)` — variadic, integer args required

### Aggregate

`sum(...)`, `max(...)`, `min(...)`, `mean(...)` / `average(...)`, `median(...)`, `mode(...)`, `product(...)`

### Statistics (≥2 args)

`std(...)` / `stddev(...)`, `std_sample(...)`, `variance(...)` / `var(...)`, `variance_sample(...)`

### Percentage

`percentof(pct, value)` — `pct / 100 * value`
`aspercent(part, whole)` — `part / whole * 100`

### Other

`clamp(x, lo, hi)`, `hypot(x, ...)`, `bin(n)`, `hex(n)`, `oct(n)`

### Prime Number Functions

`isprime(n)`, `nextprime(n)`, `prevprime(n)`, `primefactors(n)` — max n=10¹²

### Random Functions (not available in MCP mode)

`random()`, `randint(a, b)`, `randrange(n)`, `randrange(a, b)`, `uniform(a, b)`, `randn()`, `gauss(mu, sigma)`, `seed(s)`

### Memory / Variable Functions (not available in MCP mode)

`store(val)`, `recall()`, `mplus(val)`, `mminus(val)`, `mc()`, `setvar(val, id)`, `getvar(id)`, `delvar(id)`, `listvars()`, `clearvars()`

## Natural Language

The `run()` function and `math_eval` MCP tool accept English natural language:

```bash
eggsact "thirty plus five"                    # 35
eggsact "twenty times six"                   # 120
eggsact "one hundred divided by four"        # 25
eggsact "what is the square root of 144"    # 12
eggsact "calculate 2 to the power of 10"    # 1024
eggsact "50 percent of 200"                  # 100
eggsact "the sum of ten and twenty"          # 30
eggsact "two thirds"                          # 0.666...
```

Filler phrases are stripped: "what is", "calculate", "the value of", "tell me", etc.

## Constants

### Mathematical

| Constant | Symbol | Value |
|----------|--------|-------|
| `pi` | π | 3.141592653589793 |
| `e` | e | 2.718281828459045 |
| `tau` | τ | 6.283185307179586 |
| `phi` | φ | 1.618033988749895 |

### Physical

| Constant | Symbol | Value |
|----------|--------|-------|
| `c` | Speed of light | 299792458 m/s |
| `h` | Planck constant | 6.62607015e-34 J·s |
| `hbar` | Reduced Planck | 1.054571817e-34 J·s |
| `k` | Boltzmann constant | 1.380649e-23 J/K |
| `G` | Gravitational constant | 6.67430e-11 N·m²/kg² |
| `na` | Avogadro constant | 6.02214076e23 mol⁻¹ |
| `R` | Gas constant | 8.314462618 J/(mol·K) |
| `qe` | Elementary charge | 1.602176634e-19 C |
| `me` | Electron mass | 9.1093837015e-31 kg |
| `mp` | Proton mass | 1.67262192369e-27 kg |
| `mn` | Neutron mass | 1.67493e-27 kg |
| `epsilon0` | Vacuum permittivity | 8.8541878128e-12 F/m |
| `mu0` | Vacuum permeability | 1.25663706212e-6 H/m |
| `gravity` | Standard gravity | 9.80665 m/s² |
| `atm` | Standard atmosphere | 101325 Pa |

`g` is parsed as the gram unit in unit expressions. Use `gravity` or `standardgravity` for standard gravity.

## Unit Conversions

```bash
eggsact "30m to ft"       # 98.4251968503937
eggsact "1km in miles"    # 0.621371...
eggsact "72F in C"        # 22.2222...
eggsact "1024KB in MB"    # 1
eggsact "1gal in L"       # 3.78541...
eggsact "30m + 100ft"     # 60.480000000000004 m
```

Temperature conversions use offset math, not multiplicative factors.

### Supported Unit Categories

| Category | Units |
|----------|-------|
| Length | `m`, `km`, `cm`, `mm`, `in`, `ft`, `yd`, `mi`, `ly`, `au`, `pc` |
| Mass | `kg`, `g`, `mg`, `ug`, `ng`, `lb`, `oz`, `ton`, `stone` |
| Time | `s`, `ms`, `us`, `ns`, `min`, `h`, `d`, `wk`, `yr` |
| Volume | `L`, `mL`, `gal`, `qt`, `pt`, `cup`, `floz`, `tbsp`, `tsp` |
| Temperature | `C`, `F`, `K` |
| Data | `B`, `KB`, `MB`, `GB`, `TB` |
| Pressure | `Pa`, `kPa`, `MPa`, `GPa`, `bar`, `atm`, `psi` |
| Energy | `J`, `kJ`, `cal`, `kcal`, `Wh`, `kWh`, `BTU`, `eV` |
| Power | `W`, `kW`, `MW`, `GW`, `hp` |
| Force | `N`, `kN`, `dyne`, `lbf` |
| Voltage | `V`, `kV`, `mV` |
| Current | `A`, `mA` |
| Angle | `rad`, `deg` |
| Speed | `m/s`, `km/h`, `mph`, `kn`, `mach` |
| Frequency | `Hz`, `kHz`, `MHz`, `GHz`, `THz` |

Prefixed units like `kN`, `mV`, `mA` are supported. 500+ aliases are recognized including plurals, abbreviations, and alternative spellings (e.g., `litre`/`liter`).

See [calculator.md](../architecture/calculator.md) for the complete unit definition table and conversion algorithm details.
