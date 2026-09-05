use super::shared_weak_fixture::SharedWeakFixture;
use super::*;

#[test]
fn shared_weak_operations_preserve_retained_sources_and_produce_distinct_owners() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_operations();
    for _ in 0..2 {
        let verified = fixture.verify(seed.clone()).expect("valid shared/weak operations");
        let function =
            verified.modules().next().expect("module").functions().next().expect("function");
        let block = function.blocks().next().expect("block");
        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions[0].kind(), VerifiedInstructionKind::SharedClone);
        assert_eq!(instructions[1].kind(), VerifiedInstructionKind::WeakDowngrade);
        assert_eq!(instructions[2].kind(), VerifiedInstructionKind::WeakClone);
        assert_eq!(instructions[3].kind(), VerifiedInstructionKind::SharedConstruct);

        // Verify that cleanup plans match expected live drop roots
        assert_eq!(
            instructions[0]
                .derived_drop_actions()
                .map(|drop| drop.root().index())
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
        assert_eq!(
            instructions[1]
                .derived_drop_actions()
                .map(|drop| drop.root().index())
                .collect::<Vec<_>>(),
            vec![3, 2, 1, 0]
        );
        assert_eq!(
            instructions[2]
                .derived_drop_actions()
                .map(|drop| drop.root().index())
                .collect::<Vec<_>>(),
            vec![4, 3, 2, 1, 0]
        );
        assert_eq!(
            instructions[3]
                .derived_drop_actions()
                .map(|drop| drop.root().index())
                .collect::<Vec<_>>(),
            vec![5, 4, 3, 2, 1, 0]
        );

        // Verify terminator return cleanup
        assert_eq!(
            block
                .terminator()
                .derived_drop_actions()
                .map(|drop| drop.root().index())
                .collect::<Vec<_>>(),
            vec![5, 4, 3, 1, 0]
        );
    }
}

#[test]
fn shared_weak_scalar_payload_operations_verify_deterministically() {
    let fixture = SharedWeakFixture::new();
    let mut raw = program(&fixture.sources, &fixture.linear, &fixture.linux);
    let function = &mut raw.modules[0].functions[0];
    let span = function.span;
    let value = |id, ty| raw::ValueDefinition { id: raw::ValueId(id), ty, span };

    function.entry_export = None;
    function.parameters = vec![value(0, fixture.shared_i32), value(1, fixture.i32_type)];
    function.result = fixture.shared_i32;

    function.places = vec![
        raw::Place {
            id: raw::PlaceId(0),
            ty: fixture.shared_i32,
            span,
            kind: raw::PlaceKind::Parameter(0),
        },
        raw::Place {
            id: raw::PlaceId(1),
            ty: fixture.i32_type,
            span,
            kind: raw::PlaceKind::Parameter(1),
        },
        raw::Place {
            id: raw::PlaceId(2),
            ty: fixture.weak_i32,
            span,
            kind: raw::PlaceKind::Temporary(raw::ValueId(2)),
        },
        raw::Place {
            id: raw::PlaceId(3),
            ty: fixture.shared_i32,
            span,
            kind: raw::PlaceKind::Temporary(raw::ValueId(3)),
        },
    ];

    function.blocks[0].instructions = vec![
        raw::Instruction {
            result: Some(value(2, fixture.weak_i32)),
            span,
            kind: raw::InstructionKind::WeakDowngrade {
                place: raw::PlaceId(0),
                cleanup: raw::CleanupPlanId(0),
            },
        },
        raw::Instruction {
            result: Some(value(3, fixture.shared_i32)),
            span,
            kind: raw::InstructionKind::SharedConstruct {
                value: raw::ValueId(1),
                cleanup: raw::CleanupPlanId(1),
            },
        },
    ];

    function.blocks[0].terminators[0].kind =
        raw::Terminator::Return { value: raw::ValueId(3), cleanup: raw::CleanupPlanId(2) };

    function.cleanup_plans = vec![
        raw::CleanupPlan {
            id: raw::CleanupPlanId(0),
            span,
            actions: vec![raw::DropAction::DropPlace(raw::PlaceId(0))],
        },
        raw::CleanupPlan {
            id: raw::CleanupPlanId(1),
            span,
            actions: vec![
                raw::DropAction::DropPlace(raw::PlaceId(2)),
                raw::DropAction::DropPlace(raw::PlaceId(0)),
            ],
        },
        raw::CleanupPlan {
            id: raw::CleanupPlanId(2),
            span,
            actions: vec![
                raw::DropAction::DropPlace(raw::PlaceId(2)),
                raw::DropAction::DropPlace(raw::PlaceId(0)),
            ],
        },
    ];

    for _ in 0..2 {
        let verified = fixture.verify(raw.clone()).expect("valid scalar shared/weak program");
        let function =
            verified.modules().next().expect("module").functions().next().expect("function");
        let block = function.blocks().next().expect("block");
        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions[0].kind(), VerifiedInstructionKind::WeakDowngrade);
        assert_eq!(instructions[1].kind(), VerifiedInstructionKind::SharedConstruct);
    }
}
