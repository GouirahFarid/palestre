//! La machine ne panique sur aucun code, et ne dépasse jamais son budget (invariants 2 et 5).
//!
//! Le programme est construit directement, sans en-tête : le fuzzer explore le code au lieu de deviner un `.pbc`
//! valide. Les deux premiers octets donnent le budget, les suivants le code ; les réponses aux rappels sont lues
//! dans les mêmes octets.
#![no_main]
use std::num::NonZeroU16;

use libfuzzer_sys::fuzz_target;
use pbc::{Fault, Program};
use vm::{Action, Env, Machine, run_with};

struct Answers<'a>(&'a [u8], usize);

impl Answers<'_> {
    fn next(&mut self) -> i64 {
        let b = self.0.get(self.1 % self.0.len().max(1)).copied().unwrap_or(0);
        self.1 += 1;
        i64::from(b as i8)
    }
}

impl Env for Answers<'_> {
    fn sense(&mut self, _k: u8) -> Result<i64, Fault> {
        let v = self.next();
        if v == 0 { Err(Fault::BadSensor) } else { Ok(v) }
    }
    fn act(&mut self, _action: Action) -> bool {
        self.next() > 0
    }
    fn rand(&mut self) -> i64 {
        self.next()
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&[b0, b1], code)) = data.split_first_chunk::<2>() else { return };
    let budget = NonZeroU16::new(u16::from_be_bytes([b0, b1])).unwrap_or(NonZeroU16::MIN);
    let Ok(program) = Program::new(vec![0, 1, -1, i64::MIN, i64::MAX], code.to_vec()) else { return };
    let report = run_with(Machine::new(&program, [0; 256], budget), &mut Answers(data, 0));
    assert!(report.gas_used <= budget.get());
});
