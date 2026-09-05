use super::shared_weak_fixture::SharedWeakFixture;
use super::*;

#[test]
fn weak_upgrade_branch_synthesizes_shared_owner_on_success_and_preserves_weak_owner() {
    let fixture = SharedWeakFixture::new();
    let seed = fixture.seed_upgrade_branch();

    for _ in 0..2 {
        let verified = fixture.verify(seed.clone()).expect("valid upgrade branch program");
        let function =
            verified.modules().next().expect("module").functions().next().expect("function");
        let blocks = function.blocks().collect::<Vec<_>>();
        assert_eq!(blocks.len(), 3);

        // Block 0 ends with WeakUpgradeBranch
        assert_eq!(blocks[0].terminator().kind(), VerifiedTerminatorKind::WeakUpgradeBranch);

        // Block 1 is the Success block:
        // First block parameter is the synthesized Shared<String> owner
        let success_params = blocks[1].parameters().collect::<Vec<_>>();
        assert_eq!(success_params.len(), 1);
        assert_eq!(success_params[0].id().index(), 2);
        assert_eq!(success_params[0].ty().index(), fixture.shared_string.0);

        // Block 2 is the Expired block:
        // No synthesized parameter
        let expired_params = blocks[2].parameters().collect::<Vec<_>>();
        assert_eq!(expired_params.len(), 0);

        // Both blocks return cleanly
        assert_eq!(blocks[1].terminator().kind(), VerifiedTerminatorKind::Return);
        assert_eq!(blocks[2].terminator().kind(), VerifiedTerminatorKind::Return);
    }
}

#[test]
fn weak_upgrade_branch_with_forwarded_arguments_binds_correctly() {
    let fixture = SharedWeakFixture::new();
    let mut raw = program(&fixture.sources, &fixture.linear, &fixture.linux);
    let function = &mut raw.modules[0].functions[0];
    let span = function.span;
    let value = |id, ty| raw::ValueDefinition { id: raw::ValueId(id), ty, span };

    function.entry_export = None;
    function.parameters = vec![
        value(0, fixture.shared_string),
        value(1, fixture.weak_string),
        value(2, fixture.i32_type),
    ];
    function.result = fixture.i32_type;

    function.blocks = vec![
        raw::Block {
            id: raw::BlockId(0),
            parameters: vec![],
            instructions: vec![],
            terminators: vec![raw::SpannedTerminator {
                span,
                kind: raw::Terminator::WeakUpgradeBranch {
                    weak: raw::PlaceId(1),
                    success: raw::Edge {
                        target: raw::BlockId(1),
                        arguments: vec![raw::ValueId(2)],
                    },
                    expired: raw::Edge {
                        target: raw::BlockId(2),
                        arguments: vec![raw::ValueId(2)],
                    },
                    cleanup: raw::CleanupPlanId(0),
                },
            }],
        },
        raw::Block {
            id: raw::BlockId(1),
            parameters: vec![value(3, fixture.shared_string), value(4, fixture.i32_type)],
            instructions: vec![],
            terminators: vec![raw::SpannedTerminator {
                span,
                kind: raw::Terminator::Return {
                    value: raw::ValueId(4),
                    cleanup: raw::CleanupPlanId(1),
                },
            }],
        },
        raw::Block {
            id: raw::BlockId(2),
            parameters: vec![value(5, fixture.i32_type)],
            instructions: vec![],
            terminators: vec![raw::SpannedTerminator {
                span,
                kind: raw::Terminator::Return {
                    value: raw::ValueId(5),
                    cleanup: raw::CleanupPlanId(2),
                },
            }],
        },
    ];

    function.places = vec![
        raw::Place {
            id: raw::PlaceId(0),
            ty: fixture.shared_string,
            span,
            kind: raw::PlaceKind::Parameter(0),
        },
        raw::Place {
            id: raw::PlaceId(1),
            ty: fixture.weak_string,
            span,
            kind: raw::PlaceKind::Parameter(1),
        },
        raw::Place {
            id: raw::PlaceId(2),
            ty: fixture.i32_type,
            span,
            kind: raw::PlaceKind::Parameter(2),
        },
        raw::Place {
            id: raw::PlaceId(3),
            ty: fixture.shared_string,
            span,
            kind: raw::PlaceKind::Temporary(raw::ValueId(3)),
        },
    ];

    function.cleanup_plans = vec![
        raw::CleanupPlan {
            id: raw::CleanupPlanId(0),
            span,
            actions: vec![
                raw::DropAction::DropPlace(raw::PlaceId(1)),
                raw::DropAction::DropPlace(raw::PlaceId(0)),
            ],
        },
        raw::CleanupPlan {
            id: raw::CleanupPlanId(1),
            span,
            actions: vec![
                raw::DropAction::DropPlace(raw::PlaceId(3)),
                raw::DropAction::DropPlace(raw::PlaceId(1)),
                raw::DropAction::DropPlace(raw::PlaceId(0)),
            ],
        },
        raw::CleanupPlan {
            id: raw::CleanupPlanId(2),
            span,
            actions: vec![
                raw::DropAction::DropPlace(raw::PlaceId(1)),
                raw::DropAction::DropPlace(raw::PlaceId(0)),
            ],
        },
    ];

    let verified = fixture.verify(raw).expect("valid forwarded argument upgrade");
    let function = verified.modules().next().expect("module").functions().next().expect("function");
    let blocks = function.blocks().collect::<Vec<_>>();
    assert_eq!(blocks[1].parameters().count(), 2);
    assert_eq!(blocks[2].parameters().count(), 1);
}
