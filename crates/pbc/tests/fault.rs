#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use pbc::Fault;

#[test]
fn fault_names_are_the_b5_strings() {
    let names = [
        "BadConst",
        "BadOpcode",
        "Truncated",
        "PcOutOfRange",
        "BadJump",
        "StackUnderflow",
        "StackOverflow",
        "CallStackOverflow",
        "CallStackUnderflow",
        "OutOfGas",
        "DivByZero",
        "OutOfBounds",
        "BadSensor",
        "BadAction",
        "AlreadyActed",
    ];
    assert_eq!(Fault::ALL.len(), 15);
    for (fault, name) in Fault::ALL.iter().zip(names) {
        assert_eq!(fault.as_str(), name);
        assert_eq!(fault.to_string(), name);
        assert_eq!(Fault::from_name(name), Some(*fault));
    }
}

#[test]
fn load_faults_and_wrong_case_are_not_runtime_faults() {
    assert_eq!(Fault::from_name("BadHeader"), None);
    assert_eq!(Fault::from_name("badjump"), None);
    assert_eq!(Fault::from_name(""), None);
}
