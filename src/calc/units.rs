use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq)]
pub struct UnitValue {
    pub value: f64,
    pub unit: Option<String>,
}

impl UnitValue {
    pub fn new(value: f64) -> Result<Self, String> {
        if !value.is_finite() {
            return Err("UnitValue cannot be NaN or Infinity".to_string());
        }
        Ok(Self { value, unit: None })
    }

    pub fn with_unit(value: f64, unit: &str) -> Result<Self, String> {
        if !value.is_finite() {
            return Err("UnitValue cannot be NaN or Infinity".to_string());
        }
        Ok(Self {
            value,
            unit: Some(unit.to_string()),
        })
    }

    /// Create a UnitValue without checking for non-finite values.
    /// Use this only when you're certain the value is finite (e.g., after arithmetic on finite values).
    fn new_unchecked(value: f64, unit: Option<String>) -> Self {
        Self { value, unit }
    }

    pub fn convert_to(&self, target: &str) -> Result<UnitValue, String> {
        let from_unit = self.unit.as_ref().ok_or("No unit to convert from")?;

        // Temperature units use offset-based conversion, not multiplicative factors
        let from_resolved = UNIT_ALIASES
            .get(from_unit.as_str())
            .copied()
            .unwrap_or(from_unit.as_str());
        let to_resolved = UNIT_ALIASES.get(target).copied().unwrap_or(target);
        if let (Some(from_def), Some(to_def)) =
            (UNIT_BASE.get(from_resolved), UNIT_BASE.get(to_resolved))
        {
            if from_def.category == "temperature" && to_def.category == "temperature" {
                let converted = convert_temperature(self.value, from_unit, target)?;
                return UnitValue::with_unit(converted, target);
            }
        }

        let factor = get_conversion_factor(from_unit, target)?;
        UnitValue::with_unit(self.value * factor, target)
    }
}

impl std::ops::Add for UnitValue {
    type Output = Result<Self, String>;
    fn add(self, other: Self) -> Result<Self, String> {
        match (&self.unit, &other.unit) {
            (Some(a), Some(b)) if a == b => UnitValue::with_unit(self.value + other.value, a),
            (Some(a), Some(b)) => {
                let factor = get_conversion_factor(b, a)?;
                UnitValue::with_unit(self.value + other.value * factor, a)
            }
            (Some(a), None) => UnitValue::with_unit(self.value + other.value, a),
            (None, Some(b)) => UnitValue::with_unit(self.value + other.value, b),
            (None, None) => UnitValue::new(self.value + other.value),
        }
    }
}

impl std::ops::Sub for UnitValue {
    type Output = Result<Self, String>;
    fn sub(self, other: Self) -> Result<Self, String> {
        match (&self.unit, &other.unit) {
            (Some(a), Some(b)) if a == b => UnitValue::with_unit(self.value - other.value, a),
            (Some(a), Some(b)) => {
                let factor = get_conversion_factor(b, a)?;
                UnitValue::with_unit(self.value - other.value * factor, a)
            }
            (Some(a), None) => UnitValue::with_unit(self.value - other.value, a),
            (None, Some(b)) => UnitValue::with_unit(self.value - other.value, b),
            (None, None) => UnitValue::new(self.value - other.value),
        }
    }
}

impl std::ops::Mul for UnitValue {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        let unit = match (&self.unit, &other.unit) {
            (Some(a), Some(b)) if a == b => Some(format!("{}**2", a)),
            (Some(a), Some(b)) => Some(format!("{}*{}", a, b)),
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b.clone()),
            (None, None) => None,
        };
        UnitValue::new_unchecked(self.value * other.value, unit)
    }
}

impl std::ops::Div for UnitValue {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        let unit = match (&self.unit, &other.unit) {
            (Some(a), Some(b)) if a == b => None,
            (Some(a), Some(b)) => Some(format!("{}/{}", a, b)),
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(format!("1/{}", b)),
            (None, None) => None,
        };
        UnitValue::new_unchecked(self.value / other.value, unit)
    }
}

pub struct UnitDefinition {
    pub category: &'static str,
    pub to_base: f64,
}

#[doc(hidden)]
pub static UNIT_BASE: LazyLock<HashMap<&'static str, UnitDefinition>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Length - base unit: m (meter)
    m.insert(
        "m",
        UnitDefinition {
            category: "length",
            to_base: 1.0,
        },
    );
    m.insert(
        "km",
        UnitDefinition {
            category: "length",
            to_base: 1000.0,
        },
    );
    m.insert(
        "cm",
        UnitDefinition {
            category: "length",
            to_base: 0.01,
        },
    );
    m.insert(
        "mm",
        UnitDefinition {
            category: "length",
            to_base: 0.001,
        },
    );
    m.insert(
        "um",
        UnitDefinition {
            category: "length",
            to_base: 1e-6,
        },
    );
    m.insert(
        "nm",
        UnitDefinition {
            category: "length",
            to_base: 1e-9,
        },
    );
    m.insert(
        "pm",
        UnitDefinition {
            category: "length",
            to_base: 1e-12,
        },
    );
    m.insert(
        "inch",
        UnitDefinition {
            category: "length",
            to_base: 0.0254,
        },
    );
    m.insert(
        "ft",
        UnitDefinition {
            category: "length",
            to_base: 0.3048,
        },
    );
    m.insert(
        "yd",
        UnitDefinition {
            category: "length",
            to_base: 0.9144,
        },
    );
    m.insert(
        "mi",
        UnitDefinition {
            category: "length",
            to_base: 1609.344,
        },
    );
    m.insert(
        "ly",
        UnitDefinition {
            category: "length",
            to_base: 9.4607304725808e15,
        },
    );
    m.insert(
        "au",
        UnitDefinition {
            category: "length",
            to_base: 1.49597870700e11,
        },
    );
    m.insert(
        "pc",
        UnitDefinition {
            category: "length",
            to_base: 3.085_677_581_491_367e16,
        },
    );
    m.insert(
        "angstrom",
        UnitDefinition {
            category: "length",
            to_base: 1e-10,
        },
    );
    m.insert(
        "fermi",
        UnitDefinition {
            category: "length",
            to_base: 1e-15,
        },
    );
    m.insert(
        "nmi",
        UnitDefinition {
            category: "length",
            to_base: 1852.0,
        },
    );
    m.insert(
        "furlong",
        UnitDefinition {
            category: "length",
            to_base: 201.168,
        },
    );
    m.insert(
        "chain",
        UnitDefinition {
            category: "length",
            to_base: 20.1168,
        },
    );
    m.insert(
        "rod",
        UnitDefinition {
            category: "length",
            to_base: 5.0292,
        },
    );
    m.insert(
        "fathom",
        UnitDefinition {
            category: "length",
            to_base: 1.8288,
        },
    );
    m.insert(
        "smoot",
        UnitDefinition {
            category: "length",
            to_base: 1.7018,
        },
    );

    // Time - base unit: s (second)
    m.insert(
        "s",
        UnitDefinition {
            category: "time",
            to_base: 1.0,
        },
    );
    m.insert(
        "ms",
        UnitDefinition {
            category: "time",
            to_base: 0.001,
        },
    );
    m.insert(
        "us",
        UnitDefinition {
            category: "time",
            to_base: 1e-6,
        },
    );
    m.insert(
        "ns",
        UnitDefinition {
            category: "time",
            to_base: 1e-9,
        },
    );
    m.insert(
        "ps",
        UnitDefinition {
            category: "time",
            to_base: 1e-12,
        },
    );
    m.insert(
        "min",
        UnitDefinition {
            category: "time",
            to_base: 60.0,
        },
    );
    m.insert(
        "h",
        UnitDefinition {
            category: "time",
            to_base: 3600.0,
        },
    );
    m.insert(
        "d",
        UnitDefinition {
            category: "time",
            to_base: 86400.0,
        },
    );
    m.insert(
        "day",
        UnitDefinition {
            category: "time",
            to_base: 86400.0,
        },
    );
    m.insert(
        "wk",
        UnitDefinition {
            category: "time",
            to_base: 604800.0,
        },
    );
    m.insert(
        "yr",
        UnitDefinition {
            category: "time",
            to_base: 31536000.0,
        },
    );
    m.insert(
        "year",
        UnitDefinition {
            category: "time",
            to_base: 31536000.0,
        },
    );
    m.insert(
        "fortnight",
        UnitDefinition {
            category: "time",
            to_base: 1209600.0,
        },
    );
    m.insert(
        "decade",
        UnitDefinition {
            category: "time",
            to_base: 315360000.0,
        },
    );
    m.insert(
        "century",
        UnitDefinition {
            category: "time",
            to_base: 3153600000.0,
        },
    );
    m.insert(
        "millennium",
        UnitDefinition {
            category: "time",
            to_base: 31536000000.0,
        },
    );

    // Mass - base unit: kg (kilogram)
    m.insert(
        "kg",
        UnitDefinition {
            category: "mass",
            to_base: 1.0,
        },
    );
    m.insert(
        "g",
        UnitDefinition {
            category: "mass",
            to_base: 0.001,
        },
    );
    m.insert(
        "mg",
        UnitDefinition {
            category: "mass",
            to_base: 1e-6,
        },
    );
    m.insert(
        "ug",
        UnitDefinition {
            category: "mass",
            to_base: 1e-9,
        },
    );
    m.insert(
        "ng",
        UnitDefinition {
            category: "mass",
            to_base: 1e-12,
        },
    );
    m.insert(
        "lb",
        UnitDefinition {
            category: "mass",
            to_base: 0.45359237,
        },
    );
    m.insert(
        "oz",
        UnitDefinition {
            category: "mass",
            to_base: 0.028349523125,
        },
    );
    m.insert(
        "ton",
        UnitDefinition {
            category: "mass",
            to_base: 907.18474,
        },
    );
    m.insert(
        "stone",
        UnitDefinition {
            category: "mass",
            to_base: 6.35029318,
        },
    );
    m.insert(
        "tonne",
        UnitDefinition {
            category: "mass",
            to_base: 1000.0,
        },
    );
    m.insert(
        "long_ton",
        UnitDefinition {
            category: "mass",
            to_base: 1016.0469,
        },
    );
    m.insert(
        "slug",
        UnitDefinition {
            category: "mass",
            to_base: 14.593903,
        },
    );
    m.insert(
        "ct",
        UnitDefinition {
            category: "mass",
            to_base: 0.0002,
        },
    );
    m.insert(
        "gr",
        UnitDefinition {
            category: "mass",
            to_base: 6.479891e-5,
        },
    );
    m.insert(
        "dr",
        UnitDefinition {
            category: "mass",
            to_base: 0.0017718452,
        },
    );

    // Volume - base unit: L (liter)
    m.insert(
        "L",
        UnitDefinition {
            category: "volume",
            to_base: 1.0,
        },
    );
    m.insert(
        "mL",
        UnitDefinition {
            category: "volume",
            to_base: 0.001,
        },
    );
    m.insert(
        "uL",
        UnitDefinition {
            category: "volume",
            to_base: 1e-6,
        },
    );
    m.insert(
        "gal",
        UnitDefinition {
            category: "volume",
            to_base: 3.785411784,
        },
    );
    m.insert(
        "qt",
        UnitDefinition {
            category: "volume",
            to_base: 0.946352946,
        },
    );
    m.insert(
        "pt",
        UnitDefinition {
            category: "volume",
            to_base: 0.473176473,
        },
    );
    m.insert(
        "cup",
        UnitDefinition {
            category: "volume",
            to_base: 0.2365882365,
        },
    );
    m.insert(
        "floz",
        UnitDefinition {
            category: "volume",
            to_base: 0.02957352954,
        },
    );
    m.insert(
        "tbsp",
        UnitDefinition {
            category: "volume",
            to_base: 0.01478676477,
        },
    );
    m.insert(
        "tsp",
        UnitDefinition {
            category: "volume",
            to_base: 0.00492892159,
        },
    );
    // Cubic volume
    m.insert(
        "m3",
        UnitDefinition {
            category: "volume",
            to_base: 1000.0,
        },
    );
    m.insert(
        "cm3",
        UnitDefinition {
            category: "volume",
            to_base: 0.001,
        },
    );
    m.insert(
        "ft3",
        UnitDefinition {
            category: "volume",
            to_base: 28.316846592,
        },
    );
    m.insert(
        "in3",
        UnitDefinition {
            category: "volume",
            to_base: 0.016387064,
        },
    );
    m.insert(
        "yd3",
        UnitDefinition {
            category: "volume",
            to_base: 764.554857984,
        },
    );
    m.insert(
        "mm3",
        UnitDefinition {
            category: "volume",
            to_base: 1e-6,
        },
    );
    m.insert(
        "km3",
        UnitDefinition {
            category: "volume",
            to_base: 1e12,
        },
    );
    m.insert(
        "mi3",
        UnitDefinition {
            category: "volume",
            to_base: 4.168181825e12,
        },
    );

    // Data - base unit: B (byte)
    m.insert(
        "B",
        UnitDefinition {
            category: "data",
            to_base: 1.0,
        },
    );
    m.insert(
        "bit",
        UnitDefinition {
            category: "data",
            to_base: 0.125,
        },
    );
    m.insert(
        "KB",
        UnitDefinition {
            category: "data",
            to_base: 1024.0,
        },
    );
    m.insert(
        "MB",
        UnitDefinition {
            category: "data",
            to_base: 1048576.0,
        },
    );
    m.insert(
        "GB",
        UnitDefinition {
            category: "data",
            to_base: 1073741824.0,
        },
    );
    m.insert(
        "TB",
        UnitDefinition {
            category: "data",
            to_base: 1099511627776.0,
        },
    );
    m.insert(
        "PB",
        UnitDefinition {
            category: "data",
            to_base: 1125899906842624.0,
        },
    );
    m.insert(
        "EB",
        UnitDefinition {
            category: "data",
            to_base: 1152921504606846976.0,
        },
    );
    m.insert(
        "ZB",
        UnitDefinition {
            category: "data",
            to_base: 1.1805916207174113e21,
        },
    );
    m.insert(
        "YB",
        UnitDefinition {
            category: "data",
            to_base: 1.2089258196146292e24,
        },
    );

    // Data transfer rate - base unit: bps (bits per second)
    m.insert(
        "bps",
        UnitDefinition {
            category: "data_rate",
            to_base: 1.0,
        },
    );
    m.insert(
        "Kbps",
        UnitDefinition {
            category: "data_rate",
            to_base: 1000.0,
        },
    );
    m.insert(
        "Mbps",
        UnitDefinition {
            category: "data_rate",
            to_base: 1000000.0,
        },
    );
    m.insert(
        "Gbps",
        UnitDefinition {
            category: "data_rate",
            to_base: 1000000000.0,
        },
    );

    // Pressure - base unit: Pa (pascal)
    m.insert(
        "Pa",
        UnitDefinition {
            category: "pressure",
            to_base: 1.0,
        },
    );
    m.insert(
        "kPa",
        UnitDefinition {
            category: "pressure",
            to_base: 1000.0,
        },
    );
    m.insert(
        "MPa",
        UnitDefinition {
            category: "pressure",
            to_base: 1e6,
        },
    );
    m.insert(
        "GPa",
        UnitDefinition {
            category: "pressure",
            to_base: 1e9,
        },
    );
    m.insert(
        "bar",
        UnitDefinition {
            category: "pressure",
            to_base: 100000.0,
        },
    );
    m.insert(
        "mbar",
        UnitDefinition {
            category: "pressure",
            to_base: 100.0,
        },
    );
    m.insert(
        "atm",
        UnitDefinition {
            category: "pressure",
            to_base: 101325.0,
        },
    );
    m.insert(
        "psi",
        UnitDefinition {
            category: "pressure",
            to_base: 6894.757293168,
        },
    );
    m.insert(
        "mmHg",
        UnitDefinition {
            category: "pressure",
            to_base: 133.32236842105,
        },
    );
    m.insert(
        "torr",
        UnitDefinition {
            category: "pressure",
            to_base: 133.32236842105,
        },
    );
    m.insert(
        "inHg",
        UnitDefinition {
            category: "pressure",
            to_base: 3386.389,
        },
    );
    m.insert(
        "mmH2O",
        UnitDefinition {
            category: "pressure",
            to_base: 9.80665,
        },
    );
    m.insert(
        "inH2O",
        UnitDefinition {
            category: "pressure",
            to_base: 249.08891,
        },
    );

    // Energy - base unit: J (joule)
    m.insert(
        "J",
        UnitDefinition {
            category: "energy",
            to_base: 1.0,
        },
    );
    m.insert(
        "kJ",
        UnitDefinition {
            category: "energy",
            to_base: 1000.0,
        },
    );
    m.insert(
        "MJ",
        UnitDefinition {
            category: "energy",
            to_base: 1e6,
        },
    );
    m.insert(
        "GJ",
        UnitDefinition {
            category: "energy",
            to_base: 1e9,
        },
    );
    m.insert(
        "cal",
        UnitDefinition {
            category: "energy",
            to_base: 4.184,
        },
    );
    m.insert(
        "kcal",
        UnitDefinition {
            category: "energy",
            to_base: 4184.0,
        },
    );
    m.insert(
        "Wh",
        UnitDefinition {
            category: "energy",
            to_base: 3600.0,
        },
    );
    m.insert(
        "kWh",
        UnitDefinition {
            category: "energy",
            to_base: 3.6e6,
        },
    );
    m.insert(
        "BTU",
        UnitDefinition {
            category: "energy",
            to_base: 1055.05585262,
        },
    );
    m.insert(
        "eV",
        UnitDefinition {
            category: "energy",
            to_base: 1.602176634e-19,
        },
    );

    // Power - base unit: W (watt)
    m.insert(
        "W",
        UnitDefinition {
            category: "power",
            to_base: 1.0,
        },
    );
    m.insert(
        "kW",
        UnitDefinition {
            category: "power",
            to_base: 1000.0,
        },
    );
    m.insert(
        "MW",
        UnitDefinition {
            category: "power",
            to_base: 1e6,
        },
    );
    m.insert(
        "GW",
        UnitDefinition {
            category: "power",
            to_base: 1e9,
        },
    );
    m.insert(
        "mW",
        UnitDefinition {
            category: "power",
            to_base: 0.001,
        },
    );
    m.insert(
        "hp",
        UnitDefinition {
            category: "power",
            to_base: 745.699_871_582_270_2,
        },
    );

    // Force - base unit: N (newton)
    m.insert(
        "N",
        UnitDefinition {
            category: "force",
            to_base: 1.0,
        },
    );
    m.insert(
        "kN",
        UnitDefinition {
            category: "force",
            to_base: 1000.0,
        },
    );
    m.insert(
        "mN",
        UnitDefinition {
            category: "force",
            to_base: 0.001,
        },
    );
    m.insert(
        "dyne",
        UnitDefinition {
            category: "force",
            to_base: 1e-5,
        },
    );
    m.insert(
        "lbf",
        UnitDefinition {
            category: "force",
            to_base: 4.4482216152605,
        },
    );

    // Voltage - base unit: V (volt)
    m.insert(
        "V",
        UnitDefinition {
            category: "voltage",
            to_base: 1.0,
        },
    );
    m.insert(
        "kV",
        UnitDefinition {
            category: "voltage",
            to_base: 1000.0,
        },
    );
    m.insert(
        "mV",
        UnitDefinition {
            category: "voltage",
            to_base: 0.001,
        },
    );
    m.insert(
        "μV",
        UnitDefinition {
            category: "voltage",
            to_base: 1e-6,
        },
    );

    // Current - base unit: A (ampere)
    m.insert(
        "A",
        UnitDefinition {
            category: "current",
            to_base: 1.0,
        },
    );
    m.insert(
        "mA",
        UnitDefinition {
            category: "current",
            to_base: 0.001,
        },
    );
    m.insert(
        "μA",
        UnitDefinition {
            category: "current",
            to_base: 1e-6,
        },
    );

    // Angle - base unit: rad (radian)
    m.insert(
        "rad",
        UnitDefinition {
            category: "angle",
            to_base: 1.0,
        },
    );
    m.insert(
        "deg",
        UnitDefinition {
            category: "angle",
            to_base: 0.017453292519943295,
        },
    );
    m.insert(
        "°",
        UnitDefinition {
            category: "angle",
            to_base: 0.017453292519943295,
        },
    );

    // Speed - base unit: m/s
    m.insert(
        "m/s",
        UnitDefinition {
            category: "speed",
            to_base: 1.0,
        },
    );
    m.insert(
        "km/h",
        UnitDefinition {
            category: "speed",
            to_base: 1000.0 / 3600.0,
        },
    );
    m.insert(
        "mph",
        UnitDefinition {
            category: "speed",
            to_base: 0.44704,
        },
    );
    m.insert(
        "kn",
        UnitDefinition {
            category: "speed",
            to_base: 1852.0 / 3600.0,
        },
    );
    m.insert(
        "mach",
        UnitDefinition {
            category: "speed",
            to_base: 340.29,
        },
    );

    // Area - base unit: m2 (square meter)
    m.insert(
        "m2",
        UnitDefinition {
            category: "area",
            to_base: 1.0,
        },
    );
    m.insert(
        "km2",
        UnitDefinition {
            category: "area",
            to_base: 1000000.0,
        },
    );
    m.insert(
        "cm2",
        UnitDefinition {
            category: "area",
            to_base: 0.0001,
        },
    );
    m.insert(
        "mm2",
        UnitDefinition {
            category: "area",
            to_base: 1e-6,
        },
    );
    m.insert(
        "ha",
        UnitDefinition {
            category: "area",
            to_base: 10000.0,
        },
    );
    m.insert(
        "acre",
        UnitDefinition {
            category: "area",
            to_base: 4046.8564224,
        },
    );
    m.insert(
        "ft2",
        UnitDefinition {
            category: "area",
            to_base: 0.09290304,
        },
    );
    m.insert(
        "in2",
        UnitDefinition {
            category: "area",
            to_base: 0.00064516,
        },
    );
    m.insert(
        "mi2",
        UnitDefinition {
            category: "area",
            to_base: 2589988.110336,
        },
    );
    m.insert(
        "yd2",
        UnitDefinition {
            category: "area",
            to_base: 0.83612736,
        },
    );

    // Frequency - base unit: Hz
    m.insert(
        "Hz",
        UnitDefinition {
            category: "frequency",
            to_base: 1.0,
        },
    );
    m.insert(
        "kHz",
        UnitDefinition {
            category: "frequency",
            to_base: 1000.0,
        },
    );
    m.insert(
        "MHz",
        UnitDefinition {
            category: "frequency",
            to_base: 1e6,
        },
    );
    m.insert(
        "GHz",
        UnitDefinition {
            category: "frequency",
            to_base: 1e9,
        },
    );
    m.insert(
        "THz",
        UnitDefinition {
            category: "frequency",
            to_base: 1e12,
        },
    );

    // Temperature - placeholders (actual conversion via convert_temperature)
    m.insert(
        "K",
        UnitDefinition {
            category: "temperature",
            to_base: 1.0,
        },
    );
    m.insert(
        "C",
        UnitDefinition {
            category: "temperature",
            to_base: 1.0,
        },
    );
    m.insert(
        "F",
        UnitDefinition {
            category: "temperature",
            to_base: 1.0,
        },
    );
    m.insert(
        "Ra",
        UnitDefinition {
            category: "temperature",
            to_base: 1.0,
        },
    );

    m
});

#[doc(hidden)]
pub static UNIT_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Length
    m.insert("m", "m");
    m.insert("meter", "m");
    m.insert("meters", "m");
    m.insert("metre", "m");
    m.insert("metres", "m");
    m.insert("km", "km");
    m.insert("kilometer", "km");
    m.insert("kilometers", "km");
    m.insert("kilometre", "km");
    m.insert("kilometres", "km");
    m.insert("cm", "cm");
    m.insert("centimeter", "cm");
    m.insert("centimeters", "cm");
    m.insert("centimetre", "cm");
    m.insert("centimetres", "cm");
    m.insert("mm", "mm");
    m.insert("millimeter", "mm");
    m.insert("millimeters", "mm");
    m.insert("millimetre", "mm");
    m.insert("millimetres", "mm");
    m.insert("um", "um");
    m.insert("μm", "um");
    m.insert("micrometer", "um");
    m.insert("micrometers", "um");
    m.insert("nm", "nm");
    m.insert("nanometer", "nm");
    m.insert("nanometers", "nm");
    m.insert("pm", "pm");
    m.insert("picometer", "pm");
    m.insert("picometers", "pm");
    m.insert("in", "inch");
    m.insert("inches", "inch");
    m.insert("ft", "ft");
    m.insert("foot", "ft");
    m.insert("feet", "ft");
    m.insert("yd", "yd");
    m.insert("yard", "yd");
    m.insert("yards", "yd");
    m.insert("mi", "mi");
    m.insert("mile", "mi");
    m.insert("miles", "mi");
    m.insert("ly", "ly");
    m.insert("lightyear", "ly");
    m.insert("lightyears", "ly");
    m.insert("au", "au");
    m.insert("astronomicalunit", "au");
    m.insert("astronomicalunits", "au");
    m.insert("pc", "pc");
    m.insert("parsec", "pc");
    m.insert("parsecs", "pc");
    m.insert("angstrom", "angstrom");
    m.insert("angstroms", "angstrom");
    m.insert("fermi", "fermi");
    m.insert("nmi", "nmi");
    m.insert("nauticalmile", "nmi");
    m.insert("nauticalmiles", "nmi");
    m.insert("furlong", "furlong");
    m.insert("furlongs", "furlong");
    m.insert("chain", "chain");
    m.insert("chains", "chain");
    m.insert("rd", "rod");
    m.insert("rod", "rod");
    m.insert("rods", "rod");
    m.insert("fathom", "fathom");
    m.insert("fathoms", "fathom");
    m.insert("smoot", "smoot");
    m.insert("smoots", "smoot");
    // Time
    m.insert("s", "s");
    m.insert("second", "s");
    m.insert("seconds", "s");
    m.insert("sec", "s");
    m.insert("secs", "s");
    m.insert("ms", "ms");
    m.insert("millisecond", "ms");
    m.insert("milliseconds", "ms");
    m.insert("us", "us");
    m.insert("μs", "us");
    m.insert("microsecond", "us");
    m.insert("microseconds", "us");
    m.insert("ns", "ns");
    m.insert("nanosecond", "ns");
    m.insert("nanoseconds", "ns");
    m.insert("ps", "ps");
    m.insert("picosecond", "ps");
    m.insert("picoseconds", "ps");
    m.insert("min", "min");
    m.insert("minute", "min");
    m.insert("minutes", "min");
    m.insert("h", "h");
    m.insert("hr", "h");
    m.insert("hour", "h");
    m.insert("hours", "h");
    m.insert("d", "d");
    m.insert("day", "d");
    m.insert("days", "d");
    m.insert("wk", "wk");
    m.insert("week", "wk");
    m.insert("weeks", "wk");
    m.insert("yr", "yr");
    m.insert("year", "yr");
    m.insert("years", "yr");
    m.insert("fortnight", "fortnight");
    m.insert("fortnights", "fortnight");
    m.insert("decade", "decade");
    m.insert("decades", "decade");
    m.insert("century", "century");
    m.insert("centuries", "century");
    m.insert("millennium", "millennium");
    m.insert("millennia", "millennium");
    // Data storage
    m.insert("B", "B");
    m.insert("byte", "B");
    m.insert("bytes", "B");
    m.insert("bit", "bit");
    m.insert("bits", "bit");
    // BUG-207: lowercase "b" is the SI symbol for *bit*, distinct from
    // uppercase "B" (byte). Register the explicit alias so the uppercase
    // fallback doesn't alias "b" → byte.
    m.insert("b", "bit");
    m.insert("KB", "KB");
    m.insert("kilobyte", "KB");
    m.insert("kilobytes", "KB");
    m.insert("MB", "MB");
    m.insert("megabyte", "MB");
    m.insert("megabytes", "MB");
    m.insert("GB", "GB");
    m.insert("gigabyte", "GB");
    m.insert("gigabytes", "GB");
    m.insert("TB", "TB");
    m.insert("terabyte", "TB");
    m.insert("terabytes", "TB");
    m.insert("PB", "PB");
    m.insert("petabyte", "PB");
    m.insert("petabytes", "PB");
    m.insert("EB", "EB");
    m.insert("exabyte", "EB");
    m.insert("exabytes", "EB");
    m.insert("ZB", "ZB");
    m.insert("zettabyte", "ZB");
    m.insert("zettabytes", "ZB");
    m.insert("YB", "YB");
    m.insert("yottabyte", "YB");
    m.insert("yottabytes", "YB");
    // Data transfer
    m.insert("bps", "bps");
    m.insert("bit/s", "bps");
    m.insert("bits/s", "bps");
    m.insert("Kbps", "Kbps");
    m.insert("kilobps", "Kbps");
    m.insert("kilobit/s", "Kbps");
    m.insert("kilobits/s", "Kbps");
    m.insert("Mbps", "Mbps");
    m.insert("megabps", "Mbps");
    m.insert("megabit/s", "Mbps");
    m.insert("megabits/s", "Mbps");
    m.insert("Gbps", "Gbps");
    m.insert("gigabps", "Gbps");
    m.insert("gigabit/s", "Gbps");
    m.insert("gigabits/s", "Gbps");
    // Mass
    m.insert("kg", "kg");
    m.insert("kilogram", "kg");
    m.insert("kilograms", "kg");
    m.insert("g", "g");
    m.insert("gram", "g");
    m.insert("grams", "g");
    m.insert("mg", "mg");
    m.insert("milligram", "mg");
    m.insert("milligrams", "mg");
    m.insert("ug", "ug");
    m.insert("μg", "ug");
    m.insert("microgram", "ug");
    m.insert("micrograms", "ug");
    m.insert("ng", "ng");
    m.insert("nanogram", "ng");
    m.insert("nanograms", "ng");
    m.insert("lb", "lb");
    m.insert("lbs", "lb");
    m.insert("pound", "lb");
    m.insert("pounds", "lb");
    m.insert("oz", "oz");
    m.insert("ounce", "oz");
    m.insert("ounces", "oz");
    m.insert("ton", "ton");
    m.insert("tons", "ton");
    m.insert("tonne", "tonne");
    m.insert("tonnes", "tonne");
    m.insert("stone", "stone");
    m.insert("stones", "stone");
    m.insert("st", "stone");
    m.insert("long_ton", "long_ton");
    m.insert("imperial_ton", "long_ton");
    m.insert("slug", "slug");
    m.insert("slugs", "slug");
    m.insert("ct", "ct");
    m.insert("carat", "ct");
    m.insert("carats", "ct");
    m.insert("gr", "gr");
    m.insert("grain", "gr");
    m.insert("grains", "gr");
    m.insert("dr", "dr");
    m.insert("dram", "dr");
    m.insert("drams", "dr");
    // Volume
    m.insert("L", "L");
    m.insert("l", "L");
    m.insert("liter", "L");
    m.insert("liters", "L");
    m.insert("litre", "L");
    m.insert("litres", "L");
    m.insert("mL", "mL");
    m.insert("milliliter", "mL");
    m.insert("milliliters", "mL");
    m.insert("millilitre", "mL");
    m.insert("millilitres", "mL");
    m.insert("uL", "uL");
    m.insert("μL", "uL");
    m.insert("microliter", "uL");
    m.insert("microliters", "uL");
    m.insert("gal", "gal");
    m.insert("gallon", "gal");
    m.insert("gallons", "gal");
    m.insert("qt", "qt");
    m.insert("quart", "qt");
    m.insert("quarts", "qt");
    m.insert("pt", "pt");
    m.insert("pint", "pt");
    m.insert("pints", "pt");
    m.insert("cup", "cup");
    m.insert("cups", "cup");
    m.insert("floz", "floz");
    m.insert("fl oz", "floz");
    m.insert("fluidounce", "floz");
    m.insert("fluidounces", "floz");
    m.insert("tbsp", "tbsp");
    m.insert("tablespoon", "tbsp");
    m.insert("tablespoons", "tbsp");
    m.insert("tsp", "tsp");
    m.insert("teaspoon", "tsp");
    m.insert("teaspoons", "tsp");
    // Cubic volume
    m.insert("m3", "m3");
    m.insert("m^3", "m3");
    m.insert("cubicmeter", "m3");
    m.insert("cubicmeters", "m3");
    m.insert("cm3", "cm3");
    m.insert("cm^3", "cm3");
    m.insert("cc", "cm3");
    m.insert("cubiccentimeter", "cm3");
    m.insert("cubiccentimeters", "cm3");
    m.insert("ft3", "ft3");
    m.insert("ft^3", "ft3");
    m.insert("cubicfoot", "ft3");
    m.insert("cubicfeet", "ft3");
    m.insert("in3", "in3");
    m.insert("in^3", "in3");
    m.insert("cubicinch", "in3");
    m.insert("cubicinches", "in3");
    m.insert("yd3", "yd3");
    m.insert("yd^3", "yd3");
    m.insert("cubicyard", "yd3");
    m.insert("cubicyards", "yd3");
    m.insert("mm3", "mm3");
    m.insert("mm^3", "mm3");
    m.insert("cubicmillimeter", "mm3");
    m.insert("cubicmillimeters", "mm3");
    m.insert("km3", "km3");
    m.insert("km^3", "km3");
    m.insert("cubickilometer", "km3");
    m.insert("cubickilometers", "km3");
    m.insert("mi3", "mi3");
    m.insert("mi^3", "mi3");
    m.insert("cubicmile", "mi3");
    m.insert("cubicmiles", "mi3");
    // Pressure
    m.insert("Pa", "Pa");
    m.insert("pascal", "Pa");
    m.insert("pascals", "Pa");
    m.insert("kPa", "kPa");
    m.insert("kilopascal", "kPa");
    m.insert("kilopascals", "kPa");
    m.insert("MPa", "MPa");
    m.insert("megapascal", "MPa");
    m.insert("megapascals", "MPa");
    m.insert("GPa", "GPa");
    m.insert("gigapascal", "GPa");
    m.insert("gigapascals", "GPa");
    m.insert("bar", "bar");
    m.insert("bars", "bar");
    m.insert("mbar", "mbar");
    m.insert("millibar", "mbar");
    m.insert("atm", "atm");
    m.insert("atmosphere", "atm");
    m.insert("atmospheres", "atm");
    m.insert("psi", "psi");
    m.insert("psia", "psi");
    m.insert("mmHg", "mmHg");
    m.insert("torr", "torr");
    m.insert("inHg", "inHg");
    m.insert("mmH2O", "mmH2O");
    m.insert("inH2O", "inH2O");
    // Energy
    m.insert("J", "J");
    m.insert("joule", "J");
    m.insert("joules", "J");
    m.insert("kJ", "kJ");
    m.insert("kilojoule", "kJ");
    m.insert("kilojoules", "kJ");
    m.insert("MJ", "MJ");
    m.insert("megajoule", "MJ");
    m.insert("megajoules", "MJ");
    m.insert("GJ", "GJ");
    m.insert("gigajoule", "GJ");
    m.insert("gigajoules", "GJ");
    m.insert("cal", "cal");
    m.insert("calorie", "cal");
    m.insert("calories", "cal");
    m.insert("kcal", "kcal");
    m.insert("kilocalorie", "kcal");
    m.insert("kilocalories", "kcal");
    m.insert("Wh", "Wh");
    m.insert("watt-hour", "Wh");
    m.insert("watt-hours", "Wh");
    m.insert("kWh", "kWh");
    m.insert("kilowatt-hour", "kWh");
    m.insert("kilowatt-hours", "kWh");
    m.insert("BTU", "BTU");
    m.insert("btu", "BTU");
    m.insert("eV", "eV");
    m.insert("ev", "eV");
    m.insert("electronvolt", "eV");
    m.insert("electronvolts", "eV");
    // Power
    m.insert("W", "W");
    m.insert("watt", "W");
    m.insert("watts", "W");
    m.insert("kW", "kW");
    m.insert("kilowatt", "kW");
    m.insert("kilowatts", "kW");
    m.insert("MW", "MW");
    m.insert("megawatt", "MW");
    m.insert("megawatts", "MW");
    m.insert("GW", "GW");
    m.insert("gigawatt", "GW");
    m.insert("gigawatts", "GW");
    m.insert("mW", "mW");
    m.insert("milliwatt", "mW");
    m.insert("milliwatts", "mW");
    m.insert("hp", "hp");
    m.insert("horsepower", "hp");
    // Force
    m.insert("N", "N");
    m.insert("newton", "N");
    m.insert("newtons", "N");
    m.insert("kN", "kN");
    m.insert("kilonewton", "kN");
    m.insert("mN", "mN");
    m.insert("millinewton", "mN");
    m.insert("dyne", "dyne");
    m.insert("dynes", "dyne");
    m.insert("lbf", "lbf");
    m.insert("poundforce", "lbf");
    // Voltage
    m.insert("V", "V");
    m.insert("volt", "V");
    m.insert("volts", "V");
    m.insert("kV", "kV");
    m.insert("kilovolt", "kV");
    m.insert("mV", "mV");
    m.insert("millivolt", "mV");
    m.insert("uV", "μV");
    m.insert("μV", "μV");
    m.insert("microvolt", "μV");
    // Current
    m.insert("A", "A");
    m.insert("amp", "A");
    m.insert("ampere", "A");
    m.insert("amperes", "A");
    m.insert("mA", "mA");
    m.insert("milliamp", "mA");
    m.insert("milliampere", "mA");
    m.insert("uA", "μA");
    m.insert("μA", "μA");
    m.insert("microamp", "μA");
    m.insert("microampere", "μA");
    // Angles
    m.insert("rad", "rad");
    m.insert("radian", "rad");
    m.insert("radians", "rad");
    m.insert("deg", "deg");
    m.insert("degree", "deg");
    m.insert("degrees", "deg");
    // Temperature
    m.insert("K", "K");
    m.insert("kelvin", "K");
    m.insert("kelvins", "K");
    m.insert("C", "C");
    m.insert("celsius", "C");
    m.insert("centigrade", "C");
    m.insert("F", "F");
    m.insert("fahrenheit", "F");
    m.insert("Ra", "Ra");
    m.insert("rankine", "Ra");
    m.insert("degf", "F");
    m.insert("degc", "C");
    m.insert("degk", "K");
    m.insert("degr", "Ra");
    m.insert("°F", "F");
    m.insert("°C", "C");
    m.insert("°K", "K");
    m.insert("°R", "Ra");
    // Speed
    m.insert("m/s", "m/s");
    m.insert("mps", "m/s");
    m.insert("meterpersecond", "m/s");
    m.insert("meterspersecond", "m/s");
    m.insert("km/h", "km/h");
    m.insert("kph", "km/h");
    m.insert("kmh", "km/h");
    m.insert("kilometerperhour", "km/h");
    m.insert("kilometersperhour", "km/h");
    m.insert("mph", "mph");
    m.insert("mileperhour", "mph");
    m.insert("milesperhour", "mph");
    m.insert("mi/h", "mph");
    m.insert("kn", "kn");
    m.insert("knot", "kn");
    m.insert("knots", "kn");
    m.insert("kt", "kn");
    m.insert("mach", "mach");
    // Area
    m.insert("m2", "m2");
    m.insert("m^2", "m2");
    m.insert("sqm", "m2");
    m.insert("squaremeter", "m2");
    m.insert("squaremeters", "m2");
    m.insert("km2", "km2");
    m.insert("km^2", "km2");
    m.insert("squarekilometer", "km2");
    m.insert("squarekilometers", "km2");
    m.insert("cm2", "cm2");
    m.insert("cm^2", "cm2");
    m.insert("squarecentimeter", "cm2");
    m.insert("squarecentimeters", "cm2");
    m.insert("mm2", "mm2");
    m.insert("mm^2", "mm2");
    m.insert("squaremillimeter", "mm2");
    m.insert("squaremillimeters", "mm2");
    m.insert("ha", "ha");
    m.insert("hectare", "ha");
    m.insert("hectares", "ha");
    m.insert("acre", "acre");
    m.insert("acres", "acre");
    m.insert("ft2", "ft2");
    m.insert("ft^2", "ft2");
    m.insert("sqft", "ft2");
    m.insert("squarefoot", "ft2");
    m.insert("squarefeet", "ft2");
    m.insert("in2", "in2");
    m.insert("in^2", "in2");
    m.insert("sqin", "in2");
    m.insert("squareinch", "in2");
    m.insert("squareinches", "in2");
    m.insert("mi2", "mi2");
    m.insert("mi^2", "mi2");
    m.insert("sqmi", "mi2");
    m.insert("squaremile", "mi2");
    m.insert("squaremiles", "mi2");
    m.insert("yd2", "yd2");
    m.insert("yd^2", "yd2");
    m.insert("sqyd", "yd2");
    m.insert("squareyard", "yd2");
    m.insert("squareyards", "yd2");
    // Area: "**" exponent form
    m.insert("m**2", "m2");
    m.insert("cm**2", "cm2");
    m.insert("mm**2", "mm2");
    m.insert("km**2", "km2");
    m.insert("in**2", "in2");
    m.insert("ft**2", "ft2");
    m.insert("yd**2", "yd2");
    m.insert("mi**2", "mi2");
    m.insert("m**3", "m3");
    m.insert("cm**3", "cm3");
    m.insert("mm**3", "mm3");
    m.insert("km**3", "km3");
    m.insert("in**3", "in3");
    m.insert("ft**3", "ft3");
    m.insert("yd**3", "yd3");
    m.insert("mi**3", "mi3");
    // Frequency
    m.insert("Hz", "Hz");
    m.insert("hertz", "Hz");
    m.insert("kHz", "kHz");
    m.insert("kilohertz", "kHz");
    m.insert("MHz", "MHz");
    m.insert("megahertz", "MHz");
    m.insert("GHz", "GHz");
    m.insert("gigahertz", "GHz");
    m.insert("THz", "THz");
    m.insert("terahertz", "THz");
    // Case-insensitive aliases
    m.insert("KM", "km");
    m.insert("KG", "kg");
    m.insert("GHZ", "GHz");
    m.insert("KHZ", "kHz");
    m.insert("MHZ", "MHz");
    m.insert("Meters", "m");
    m.insert("Miles", "mi");
    m.insert("Inches", "inch");
    m.insert("Feet", "ft");
    m.insert("Pounds", "lb");
    m.insert("Ounces", "oz");
    m.insert("Celsius", "C");
    m.insert("Fahrenheit", "F");
    m.insert("Kelvin", "K");
    m.insert("Hours", "h");
    m.insert("Minutes", "min");
    m.insert("Seconds", "s");
    m.insert("Kilograms", "kg");
    m.insert("Grams", "g");
    m.insert("Liters", "L");
    m.insert("Newtons", "N");
    m.insert("Volts", "V");
    m.insert("Amps", "A");
    m.insert("Amperes", "A");
    m.insert("Watts", "W");
    m.insert("Joules", "J");
    m.insert("Pascals", "Pa");
    m
});

pub fn get_conversion_factor(from: &str, to: &str) -> Result<f64, String> {
    let from = UNIT_ALIASES.get(from).copied().unwrap_or(from);
    let to = UNIT_ALIASES.get(to).copied().unwrap_or(to);

    if from == to {
        return Ok(1.0);
    }

    let from_def = UNIT_BASE
        .get(from)
        .ok_or_else(|| format!("Unknown unit: {}", from))?;
    let to_def = UNIT_BASE
        .get(to)
        .ok_or_else(|| format!("Unknown unit: {}", to))?;

    if from_def.category != to_def.category {
        return Err(format!(
            "Cannot convert between incompatible categories: {} ({}) -> {} ({})",
            from_def.category, from, to_def.category, to
        ));
    }

    // Temperature requires offset-based conversion, not multiplicative
    if from_def.category == "temperature" {
        return Err(format!(
            "Temperature conversion requires convert_temperature(), not get_conversion_factor(). Use: convert_temperature(value, \"{}\", \"{}\")",
            from, to
        ));
    }

    Ok(from_def.to_base / to_def.to_base)
}

pub fn is_unit(unit: &str) -> bool {
    // Try exact match first
    if let Some(normalized) = UNIT_ALIASES.get(unit) {
        return UNIT_BASE.contains_key(*normalized);
    }
    if UNIT_BASE.contains_key(unit) {
        return true;
    }
    // Try case variations as fallback
    let lower = unit.to_lowercase();
    if let Some(normalized) = UNIT_ALIASES.get(lower.as_str()) {
        return UNIT_BASE.contains_key(*normalized);
    }
    if UNIT_BASE.contains_key(lower.as_str()) {
        return true;
    }
    let upper = unit.to_uppercase();
    if let Some(normalized) = UNIT_ALIASES.get(upper.as_str()) {
        return UNIT_BASE.contains_key(*normalized);
    }
    if UNIT_BASE.contains_key(upper.as_str()) {
        return true;
    }
    // Title case: capitalize first letter of each word
    let title: String = unit
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().to_string();
                    let rest: String = chars.collect::<String>().to_lowercase();
                    format!("{}{}", upper, rest)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(normalized) = UNIT_ALIASES.get(title.as_str()) {
        return UNIT_BASE.contains_key(*normalized);
    }
    if UNIT_BASE.contains_key(title.as_str()) {
        return true;
    }
    // Capitalize: capitalize first letter only
    let mut chars = unit.chars();
    let capitalize = match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().to_string();
            let rest: String = chars.collect::<String>().to_lowercase();
            format!("{}{}", upper, rest)
        }
    };
    if let Some(normalized) = UNIT_ALIASES.get(capitalize.as_str()) {
        return UNIT_BASE.contains_key(*normalized);
    }
    if UNIT_BASE.contains_key(capitalize.as_str()) {
        return true;
    }
    false
}

pub fn get_unit_info(unit: &str) -> Option<(String, &'static str)> {
    let normalized = UNIT_ALIASES.get(unit).copied().unwrap_or(unit);
    UNIT_BASE
        .get(normalized)
        .map(|def| (normalized.to_string(), def.category))
}

#[derive(Debug, Clone)]
pub struct PhysicalConstant {
    pub value: f64,
    pub symbol: &'static str,
    pub display_name: &'static str,
}

#[doc(hidden)]
pub static PHYSICAL_CONSTANTS: LazyLock<HashMap<&'static str, PhysicalConstant>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert(
            "pi",
            PhysicalConstant {
                value: std::f64::consts::PI,
                symbol: "π",
                display_name: "Pi",
            },
        );
        m.insert(
            "e",
            PhysicalConstant {
                value: std::f64::consts::E,
                symbol: "e",
                display_name: "Euler's number",
            },
        );
        m.insert(
            "tau",
            PhysicalConstant {
                value: std::f64::consts::TAU,
                symbol: "τ",
                display_name: "Tau",
            },
        );
        m.insert(
            "na",
            PhysicalConstant {
                value: 6.02214076e23,
                symbol: "N_A",
                display_name: "Avogadro constant",
            },
        );
        m.insert(
            "avogadro",
            PhysicalConstant {
                value: 6.02214076e23,
                symbol: "N_A",
                display_name: "Avogadro constant",
            },
        );
        m.insert(
            "avogadros",
            PhysicalConstant {
                value: 6.02214076e23,
                symbol: "N_A",
                display_name: "Avogadro constant",
            },
        );
        m.insert(
            "r",
            PhysicalConstant {
                value: 8.314462618,
                symbol: "R",
                display_name: "Gas constant",
            },
        );
        m.insert(
            "R",
            PhysicalConstant {
                value: 8.314462618,
                symbol: "R",
                display_name: "Gas constant",
            },
        );
        m.insert(
            "gasconstant",
            PhysicalConstant {
                value: 8.314462618,
                symbol: "R",
                display_name: "Gas constant",
            },
        );
        m.insert(
            "idealgasconstant",
            PhysicalConstant {
                value: 8.314462618,
                symbol: "R",
                display_name: "Gas constant",
            },
        );
        m.insert(
            "h",
            PhysicalConstant {
                value: 6.62607015e-34,
                symbol: "h",
                display_name: "Planck constant",
            },
        );
        m.insert(
            "planck",
            PhysicalConstant {
                value: 6.62607015e-34,
                symbol: "h",
                display_name: "Planck constant",
            },
        );
        m.insert(
            "planckconstant",
            PhysicalConstant {
                value: 6.62607015e-34,
                symbol: "h",
                display_name: "Planck constant",
            },
        );
        m.insert(
            "k",
            PhysicalConstant {
                value: 1.380649e-23,
                symbol: "k_B",
                display_name: "Boltzmann constant",
            },
        );
        m.insert(
            "boltzmann",
            PhysicalConstant {
                value: 1.380649e-23,
                symbol: "k_B",
                display_name: "Boltzmann constant",
            },
        );
        m.insert(
            "boltzmannconstant",
            PhysicalConstant {
                value: 1.380649e-23,
                symbol: "k_B",
                display_name: "Boltzmann constant",
            },
        );
        m.insert(
            "c",
            PhysicalConstant {
                value: 299792458.0,
                symbol: "c",
                display_name: "Speed of light in vacuum",
            },
        );
        m.insert(
            "c0",
            PhysicalConstant {
                value: 299792458.0,
                symbol: "c",
                display_name: "Speed of light in vacuum",
            },
        );
        m.insert(
            "speedoflight",
            PhysicalConstant {
                value: 299792458.0,
                symbol: "c",
                display_name: "Speed of light in vacuum",
            },
        );
        m.insert(
            "speedoflightvacuum",
            PhysicalConstant {
                value: 299792458.0,
                symbol: "c",
                display_name: "Speed of light in vacuum",
            },
        );
        m.insert(
            "elementarycharge",
            PhysicalConstant {
                value: 1.602176634e-19,
                symbol: "e",
                display_name: "Elementary charge",
            },
        );
        m.insert(
            "echarge",
            PhysicalConstant {
                value: 1.602176634e-19,
                symbol: "e",
                display_name: "Elementary charge",
            },
        );
        m.insert(
            "f",
            PhysicalConstant {
                value: 96485.33212,
                symbol: "F",
                display_name: "Faraday constant",
            },
        );
        m.insert(
            "faraday",
            PhysicalConstant {
                value: 96485.33212,
                symbol: "F",
                display_name: "Faraday constant",
            },
        );
        m.insert(
            "faradayconstant",
            PhysicalConstant {
                value: 96485.33212,
                symbol: "F",
                display_name: "Faraday constant",
            },
        );
        m.insert(
            "u",
            PhysicalConstant {
                value: 1.66053906660e-27,
                symbol: "u",
                display_name: "Atomic mass unit",
            },
        );
        m.insert(
            "amu",
            PhysicalConstant {
                value: 1.66053906660e-27,
                symbol: "u",
                display_name: "Atomic mass unit",
            },
        );
        m.insert(
            "atomicmassunit",
            PhysicalConstant {
                value: 1.66053906660e-27,
                symbol: "u",
                display_name: "Atomic mass unit",
            },
        );
        m.insert(
            "epsilon0",
            PhysicalConstant {
                value: 8.8541878128e-12,
                symbol: "ε₀",
                display_name: "Vacuum permittivity",
            },
        );
        m.insert(
            "vacuumpermittivity",
            PhysicalConstant {
                value: 8.8541878128e-12,
                symbol: "ε₀",
                display_name: "Vacuum permittivity",
            },
        );
        m.insert(
            "mu0",
            PhysicalConstant {
                value: 1.25663706212e-6,
                symbol: "μ₀",
                display_name: "Vacuum permeability",
            },
        );
        m.insert(
            "vacuumpermeability",
            PhysicalConstant {
                value: 1.25663706212e-6,
                symbol: "μ₀",
                display_name: "Vacuum permeability",
            },
        );
        m.insert(
            "standardgravity",
            PhysicalConstant {
                value: 9.80665,
                symbol: "gₙ",
                display_name: "Standard gravity",
            },
        );
        m.insert(
            "G",
            PhysicalConstant {
                value: 6.67430e-11,
                symbol: "G",
                display_name: "Gravitational constant",
            },
        );
        m.insert(
            "gravitationalconstant",
            PhysicalConstant {
                value: 6.67430e-11,
                symbol: "G",
                display_name: "Gravitational constant",
            },
        );
        m.insert(
            "rydberg",
            PhysicalConstant {
                value: 10973731.568160,
                symbol: "R∞",
                display_name: "Rydberg constant",
            },
        );
        m.insert(
            "rydbergconstant",
            PhysicalConstant {
                value: 10973731.568160,
                symbol: "R∞",
                display_name: "Rydberg constant",
            },
        );
        m.insert(
            "stefan",
            PhysicalConstant {
                value: 5.670374419e-8,
                symbol: "σ",
                display_name: "Stefan-Boltzmann constant",
            },
        );
        m.insert(
            "stefanboltzmann",
            PhysicalConstant {
                value: 5.670374419e-8,
                symbol: "σ",
                display_name: "Stefan-Boltzmann constant",
            },
        );
        m.insert(
            "planckbar",
            PhysicalConstant {
                value: 1.054571817e-34,
                symbol: "ℏ",
                display_name: "Reduced Planck constant",
            },
        );
        m.insert(
            "hbar",
            PhysicalConstant {
                value: 1.054571817e-34,
                symbol: "ℏ",
                display_name: "Reduced Planck constant",
            },
        );
        m.insert(
            "reducedplanck",
            PhysicalConstant {
                value: 1.054571817e-34,
                symbol: "ℏ",
                display_name: "Reduced Planck constant",
            },
        );
        m.insert(
            "me",
            PhysicalConstant {
                value: 9.1093837015e-31,
                symbol: "mₑ",
                display_name: "Electron mass",
            },
        );
        m.insert(
            "electronmass",
            PhysicalConstant {
                value: 9.1093837015e-31,
                symbol: "mₑ",
                display_name: "Electron mass",
            },
        );
        m.insert(
            "mp",
            PhysicalConstant {
                value: 1.67262192369e-27,
                symbol: "mₚ",
                display_name: "Proton mass",
            },
        );
        m.insert(
            "protonmass",
            PhysicalConstant {
                value: 1.67262192369e-27,
                symbol: "mₚ",
                display_name: "Proton mass",
            },
        );
        m.insert(
            "mn",
            PhysicalConstant {
                value: 1.67493e-27,
                symbol: "mₙ",
                display_name: "Neutron mass",
            },
        );
        m.insert(
            "neutronmass",
            PhysicalConstant {
                value: 1.67493e-27,
                symbol: "mₙ",
                display_name: "Neutron mass",
            },
        );
        m.insert(
            "re",
            PhysicalConstant {
                value: 2.8179403262e-15,
                symbol: "rₑ",
                display_name: "Classical electron radius",
            },
        );
        m.insert(
            "electronradius",
            PhysicalConstant {
                value: 2.8179403262e-15,
                symbol: "rₑ",
                display_name: "Classical electron radius",
            },
        );
        m.insert(
            "alpha",
            PhysicalConstant {
                value: 7.2973525693e-3,
                symbol: "α",
                display_name: "Fine-structure constant",
            },
        );
        m.insert(
            "finestructure",
            PhysicalConstant {
                value: 7.2973525693e-3,
                symbol: "α",
                display_name: "Fine-structure constant",
            },
        );
        m.insert(
            "wien",
            PhysicalConstant {
                value: 2.897771955e-3,
                symbol: "b",
                display_name: "Wien displacement constant",
            },
        );
        m.insert(
            "wienconstant",
            PhysicalConstant {
                value: 2.897771955e-3,
                symbol: "b",
                display_name: "Wien displacement constant",
            },
        );
        m
    });

pub fn convert_temperature(value: f64, from: &str, to: &str) -> Result<f64, String> {
    let from = UNIT_ALIASES.get(from).copied().unwrap_or(from);
    let to = UNIT_ALIASES.get(to).copied().unwrap_or(to);
    let (from, to) = (from.to_uppercase(), to.to_uppercase());

    if !value.is_finite() {
        return Err("Temperature value must be a finite number".to_string());
    }

    if from == to {
        return Ok(value);
    }

    let celsius = match from.as_str() {
        "C" | "CELSIUS" => value,
        "F" | "FAHRENHEIT" => (value - 32.0) * 5.0 / 9.0,
        "K" | "KELVIN" => value - 273.15,
        "RA" | "RANKINE" => (value - 491.67) * 5.0 / 9.0,
        _ => return Err(format!("Unknown temperature unit: {}", from)),
    };

    let result = match to.as_str() {
        "C" | "CELSIUS" => celsius,
        "F" | "FAHRENHEIT" => celsius * 9.0 / 5.0 + 32.0,
        "K" | "KELVIN" => celsius + 273.15,
        "RA" | "RANKINE" => (celsius + 273.15) * 9.0 / 5.0,
        _ => return Err(format!("Unknown temperature unit: {}", to)),
    };

    Ok(result)
}
