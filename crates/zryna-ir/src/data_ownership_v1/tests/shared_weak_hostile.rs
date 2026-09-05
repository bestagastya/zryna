use super::shared_weak_fixture::SharedWeakFixture;
use super::*;

#[test]
fn shared_weak_rejects_wrong_types_and_mismatched_referents() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_operations();

    // Case 0: SharedClone with Weak place -> ZRYNA-I3005
    // Case 1: WeakDowngrade with Weak place -> ZRYNA-I3005
    // Case 2: WeakClone with Shared place -> ZRYNA-I3005
    // Case 3: SharedConstruct with wrong payload type -> ZRYNA-I3005
    for case in 0..4 {
        let mut raw = seed.clone();
        let function = &mut raw.modules[0].functions[0];
        let instructions = &mut function.blocks[0].instructions;

        match case {
            0 => {
                instructions[0].kind = raw::InstructionKind::SharedClone {
                    place: raw::PlaceId(1), // place 1 is Weak, not Shared
                    cleanup: raw::CleanupPlanId(0),
                };
            }
            1 => {
                instructions[1].kind = raw::InstructionKind::WeakDowngrade {
                    place: raw::PlaceId(1), // place 1 is Weak, not Shared
                    cleanup: raw::CleanupPlanId(1),
                };
            }
            2 => {
                instructions[2].kind = raw::InstructionKind::WeakClone {
                    place: raw::PlaceId(0), // place 0 is Shared, not Weak
                    cleanup: raw::CleanupPlanId(2),
                };
            }
            3 => {
                // Change result of SharedConstruct to Shared<i32> while input is String
                instructions[3].result.as_mut().expect("result").ty = fixture.shared_i32;
                function.places[6].ty = fixture.shared_i32;
                function.result = fixture.shared_i32;
            }
            _ => unreachable!(),
        }

        let first = fixture.verify(raw.clone()).expect_err("forged type claim must fail");
        assert!(
            first.iter().any(|error| error.code() == "ZRYNA-I3005"),
            "case {case} failed to emit ZRYNA-I3005: {first:?}"
        );
        assert_eq!(first, fixture.verify(raw).expect_err("deterministic repeat"));
        fixture.verify(seed.clone()).expect("recovery after rejection");
    }
}

#[test]
fn weak_upgrade_branch_rejects_forged_shapes_and_mismatched_payloads() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_upgrade_branch();

    // Case 0: Upgrade on Shared place instead of Weak -> ZRYNA-I3014
    // Case 1: Success block first parameter has wrong referent type (Shared<i32> instead of Shared<String>) -> ZRYNA-I3014
    // Case 2: Success block missing synthesized parameter -> ZRYNA-I3014
    // Case 3: Success block first parameter has Weak category instead of Shared -> ZRYNA-I3014
    for case in 0..4 {
        let mut raw = seed.clone();
        let function = &mut raw.modules[0].functions[0];

        match case {
            0 => {
                let raw::Terminator::WeakUpgradeBranch { weak, .. } =
                    &mut function.blocks[0].terminators[0].kind
                else {
                    panic!("terminator")
                };
                *weak = raw::PlaceId(0); // place 0 is Shared, not Weak
            }
            1 => {
                // Mutate synthesized param and its place to Shared<i32>
                function.blocks[1].parameters[0].ty = fixture.shared_i32;
                function.places[2].ty = fixture.shared_i32;
                let raw::Terminator::Return { value, .. } =
                    &mut function.blocks[1].terminators[0].kind
                else {
                    panic!("return")
                };
                *value = raw::ValueId(0);
            }
            2 => {
                // Empty success parameters
                function.blocks[1].parameters.clear();
                function.places.pop();
                let raw::Terminator::Return { value, .. } =
                    &mut function.blocks[1].terminators[0].kind
                else {
                    panic!("return")
                };
                *value = raw::ValueId(0);
            }
            3 => {
                // Success block first parameter has Weak category instead of Shared
                function.blocks[1].parameters[0].ty = fixture.weak_string;
                function.places[2].ty = fixture.weak_string;
                let raw::Terminator::Return { value, .. } =
                    &mut function.blocks[1].terminators[0].kind
                else {
                    panic!("return")
                };
                *value = raw::ValueId(0);
            }
            _ => unreachable!(),
        }

        let first = fixture.verify(raw.clone()).expect_err("forged upgrade branch must fail");
        assert!(
            first.iter().any(|error| error.code() == "ZRYNA-I3014"),
            "case {case} failed to emit ZRYNA-I3014: {first:?}"
        );
        assert_eq!(first, fixture.verify(raw).expect_err("deterministic repeat"));
        fixture.verify(seed.clone()).expect("recovery after rejection");
    }
}

#[test]
fn weak_upgrade_branch_rejects_expired_edge_forged_shared_owner_and_extra_parameters() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_upgrade_branch();

    // Expired block declares a parameter expecting a synthesized Shared<String> owner,
    // but WeakUpgradeBranch only synthesizes an owner on the success edge.
    let mut raw = seed.clone();
    let function = &mut raw.modules[0].functions[0];
    let span = function.span;
    function.blocks[2].parameters.push(raw::ValueDefinition {
        id: raw::ValueId(3),
        ty: fixture.shared_string,
        span,
    });
    function.places.push(raw::Place {
        id: raw::PlaceId(3),
        ty: fixture.shared_string,
        span,
        kind: raw::PlaceKind::Temporary(raw::ValueId(3)),
    });
    if let raw::Terminator::Return { value, .. } = &mut function.blocks[2].terminators[0].kind {
        *value = raw::ValueId(3);
    }

    let diagnostics =
        fixture.verify(raw.clone()).expect_err("expired edge forged parameter must fail");
    assert!(
        diagnostics.iter().any(|d| d.code() == "ZRYNA-I3007"),
        "must emit ZRYNA-I3007: {diagnostics:?}"
    );
    assert_eq!(diagnostics, fixture.verify(raw).expect_err("deterministic repeat"));
    fixture.verify(seed).expect("clean recovery");
}

#[test]
fn shared_weak_rejects_invalid_cleanup_plan_and_place_ids() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_operations();

    // Case 0: Terminator references out-of-range cleanup plan -> ZRYNA-I3007
    // Case 1: Cleanup plan drops invalid place ID -> ZRYNA-I3012
    for case in 0..2 {
        let mut raw = seed.clone();
        let function = &mut raw.modules[0].functions[0];

        let expected_code = match case {
            0 => {
                let raw::Terminator::Return { cleanup, .. } =
                    &mut function.blocks[0].terminators[0].kind
                else {
                    panic!("return")
                };
                *cleanup = raw::CleanupPlanId(99);
                "ZRYNA-I3007"
            }
            1 => {
                function.cleanup_plans[0].actions[0] = raw::DropAction::DropPlace(raw::PlaceId(99));
                "ZRYNA-I3012"
            }
            _ => unreachable!(),
        };

        let first = fixture.verify(raw.clone()).expect_err("cleanup error must fail");
        assert!(
            first.iter().any(|error| error.code() == expected_code),
            "case {case} failed to emit {expected_code}: {first:?}"
        );
        assert_eq!(first, fixture.verify(raw).expect_err("deterministic repeat"));
        fixture.verify(seed.clone()).expect("recovery after rejection");
    }
}
