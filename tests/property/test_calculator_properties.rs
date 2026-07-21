use eggsact::calc::normalize::normalize;
use eggsact::{evaluate, evaluate_with_context, split_at_operators, EvalContext};

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xff) as u8
    }
    fn ascii_string(&mut self, max_len: usize) -> String {
        let len = (self.next_u64() as usize % max_len) + 1;
        let chars: Vec<u8> = (0..len).map(|_| (self.next_u8() % 94) + 33).collect();
        String::from_utf8(chars).unwrap_or_default()
    }
}

#[test]
fn calculator_determinism() {
    let exprs = [
        "1+2",
        "2**10",
        "5 feet to meters",
        "(3+4)*2",
        "10/3",
        "5^3",
        "sqrt(2)",
    ];
    for expr in &exprs {
        let r1 = evaluate(expr);
        let r2 = evaluate(expr);
        assert_eq!(r1.is_err(), r2.is_err());
        if let (Ok(v1), Ok(v2)) = (r1, r2) {
            assert_eq!(v1, v2);
        }
    }
}

#[test]
fn calculator_context_determinism() {
    let exprs = ["1+2", "rand()", "memory(1)", "2**10"];
    for expr in &exprs {
        let mut ctx1 = EvalContext::new().with_prng_state(42);
        let mut ctx2 = EvalContext::new().with_prng_state(42);
        let r1 = evaluate_with_context(expr, &mut ctx1);
        let r2 = evaluate_with_context(expr, &mut ctx2);
        assert_eq!(r1.is_err(), r2.is_err());
        if let (Ok(v1), Ok(v2)) = (r1, r2) {
            assert_eq!(v1, v2);
        }
    }
}

#[test]
fn calculator_no_panic_on_fuzz_input() {
    let mut rng = Rng::new(0xDEAD);
    for _ in 0..200 {
        let expr = rng.ascii_string(200);
        let _ = evaluate(&expr);
    }
}

#[test]
fn calculator_error_structured() {
    let bad = ["", "+++", "(((", "1/0", "unknown_func(1)", "1 2 3"];
    for expr in &bad {
        match evaluate(expr) {
            Ok(_) => {}
            Err(e) => {
                assert!(!e.is_empty());
                assert!(e.len() < 10_000);
            }
        }
    }
}

#[test]
fn normalization_idempotent() {
    let exprs = ["1+2", " 1 + 2 ", "1+2*3", "(a+b)*c", "2**10"];
    for expr in &exprs {
        if let Ok(norm1) = normalize(expr) {
            if let Ok(norm2) = normalize(&norm1) {
                assert_eq!(norm1, norm2, "Normalization not idempotent for: {}", expr);
            }
        }
    }
}

#[test]
fn normalization_deterministic() {
    let mut rng = Rng::new(42);
    for _ in 0..100 {
        let expr = rng.ascii_string(50);
        let r1 = normalize(&expr);
        let r2 = normalize(&expr);
        assert_eq!(r1.is_err(), r2.is_err());
        if let (Ok(v1), Ok(v2)) = (r1, r2) {
            assert_eq!(v1, v2);
        }
    }
}

#[test]
fn split_at_operators_deterministic() {
    let exprs = ["1+2", "2**10", "(3+4)*2", "5 feet to meters"];
    for expr in &exprs {
        let s1 = split_at_operators(expr);
        let s2 = split_at_operators(expr);
        assert_eq!(s1, s2);
    }
}

#[test]
fn split_at_operators_returns_valid_tokens() {
    let mut rng = Rng::new(99);
    for _ in 0..100 {
        let expr = rng.ascii_string(100);
        let tokens = split_at_operators(&expr);
        assert!(!tokens.is_empty());
    }
}

#[test]
fn mcp_mode_restrictions() {
    let exprs = ["rand()", "memory(1)", "pi", "e"];
    for expr in &exprs {
        let mut ctx = EvalContext::mcp_mode();
        let _ = evaluate_with_context(expr, &mut ctx);
    }
}
