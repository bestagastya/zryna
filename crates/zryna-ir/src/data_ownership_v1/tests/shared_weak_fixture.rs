use super::*;
use zryna_layout::{StorageTarget, TypeCategory, VerifiedLayouts, raw as raw_layout};
use zryna_source::{SourceFileInput, SourceMap};

pub(super) struct SharedWeakFixture {
    pub(super) sources: SourceMap,
    pub(super) linear: VerifiedLayouts,
    pub(super) linux: VerifiedLayouts,
    pub(super) string_type: raw::TypeId,
    pub(super) i32_type: raw::TypeId,
    pub(super) shared_string: raw::TypeId,
    pub(super) weak_string: raw::TypeId,
    pub(super) shared_i32: raw::TypeId,
    pub(super) weak_i32: raw::TypeId,
}

impl SharedWeakFixture {
    pub(super) fn new() -> Self {
        let sources = SourceMap::build(vec![SourceFileInput {
            path: "main.zry".into(),
            text: "export function id(value: i32): i32 { return value; }".into(),
        }])
        .expect("source map");
        let file = sources.verify_file_id(0).expect("file");
        let field = |ordinal, ty| raw_layout::Field { ordinal, ty: raw_layout::NodeId(ty) };
        let kinds = vec![
            raw_layout::TypeKind::Bool,
            raw_layout::TypeKind::I32,
            raw_layout::TypeKind::String,
            raw_layout::TypeKind::Shared { payload: raw_layout::NodeId(2) },
            raw_layout::TypeKind::Weak { payload: raw_layout::NodeId(2) },
            raw_layout::TypeKind::Shared { payload: raw_layout::NodeId(1) },
            raw_layout::TypeKind::Weak { payload: raw_layout::NodeId(1) },
            raw_layout::TypeKind::Struct {
                module: raw_layout::ModuleId(0),
                declaration: 0,
                fields: vec![field(0, 3), field(1, 4), field(2, 1)],
            },
            raw_layout::TypeKind::Enum {
                module: raw_layout::ModuleId(0),
                declaration: 1,
                variants: vec![
                    raw_layout::Variant { ordinal: 0, payload: Some(raw_layout::NodeId(7)) },
                    raw_layout::Variant { ordinal: 1, payload: None },
                ],
            },
            raw_layout::TypeKind::FixedArray { element: raw_layout::NodeId(3), length: 2 },
            raw_layout::TypeKind::Vec { element: raw_layout::NodeId(3) },
        ];
        let graph = raw_layout::Graph {
            modules: vec![raw_layout::Module {
                id: raw_layout::ModuleId(0),
                source_file: file,
                data_declarations: 2,
            }],
            types: kinds
                .into_iter()
                .enumerate()
                .map(|(id, kind)| raw_layout::TypeNode {
                    id: raw_layout::NodeId(u32::try_from(id).expect("small graph")),
                    span: match id {
                        7 => Some(sources.span(file, 0, 6).expect("struct span")),
                        8 => Some(sources.span(file, 7, 13).expect("enum span")),
                        _ => None,
                    },
                    kind,
                })
                .collect(),
            program_roots: vec![
                raw_layout::NodeId(3),
                raw_layout::NodeId(4),
                raw_layout::NodeId(5),
                raw_layout::NodeId(6),
                raw_layout::NodeId(7),
                raw_layout::NodeId(8),
                raw_layout::NodeId(9),
                raw_layout::NodeId(10),
            ],
        };
        let linear = zryna_layout::verify(&graph, &sources, StorageTarget::Linear32V1)
            .expect("linear layout");
        let linux = zryna_layout::verify(&graph, &sources, StorageTarget::LinuxX8664V1)
            .expect("native layout");

        let find_type = |predicate: &dyn Fn(&zryna_layout::VerifiedType<'_>) -> bool| {
            raw::TypeId(linear.types().find(|ty| predicate(ty)).expect("type").id().index())
        };

        let i32_type = find_type(&|ty| ty.category() == TypeCategory::I32);
        let string_type = find_type(&|ty| ty.category() == TypeCategory::String);
        let shared_string = find_type(&|ty| {
            ty.category() == TypeCategory::Shared
                && ty.referenced_type().is_some_and(|r| r.index() == 2)
        });
        let weak_string = find_type(&|ty| {
            ty.category() == TypeCategory::Weak
                && ty.referenced_type().is_some_and(|r| r.index() == 2)
        });
        let shared_i32 = find_type(&|ty| {
            ty.category() == TypeCategory::Shared
                && ty.referenced_type().is_some_and(|r| r.index() == 1)
        });
        let weak_i32 = find_type(&|ty| {
            ty.category() == TypeCategory::Weak
                && ty.referenced_type().is_some_and(|r| r.index() == 1)
        });

        Self {
            sources,
            linear,
            linux,
            string_type,
            i32_type,
            shared_string,
            weak_string,
            shared_i32,
            weak_i32,
        }
    }

    pub(super) fn seed_operations(&self) -> raw::Program {
        let mut raw = program(&self.sources, &self.linear, &self.linux);
        let function = &mut raw.modules[0].functions[0];
        let span = function.span;
        let value = |id, ty| raw::ValueDefinition { id: raw::ValueId(id), ty, span };

        function.entry_export = None;
        function.parameters = vec![
            value(0, self.shared_string),
            value(1, self.weak_string),
            value(2, self.string_type),
        ];
        function.result = self.shared_string;

        let value_types = [
            self.shared_string, // 0 param
            self.weak_string,   // 1 param
            self.string_type,   // 2 param
            self.shared_string, // 3 SharedClone(0)
            self.weak_string,   // 4 WeakDowngrade(0)
            self.weak_string,   // 5 WeakClone(1)
            self.shared_string, // 6 SharedConstruct(2)
        ];

        function.places = value_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                let id = u32::try_from(index).expect("small arena");
                raw::Place {
                    id: raw::PlaceId(id),
                    ty,
                    span,
                    kind: if id < 3 {
                        raw::PlaceKind::Parameter(id)
                    } else {
                        raw::PlaceKind::Temporary(raw::ValueId(id))
                    },
                }
            })
            .collect();

        function.blocks[0].instructions = vec![
            raw::Instruction {
                result: Some(value(3, self.shared_string)),
                span,
                kind: raw::InstructionKind::SharedClone {
                    place: raw::PlaceId(0),
                    cleanup: raw::CleanupPlanId(0),
                },
            },
            raw::Instruction {
                result: Some(value(4, self.weak_string)),
                span,
                kind: raw::InstructionKind::WeakDowngrade {
                    place: raw::PlaceId(0),
                    cleanup: raw::CleanupPlanId(1),
                },
            },
            raw::Instruction {
                result: Some(value(5, self.weak_string)),
                span,
                kind: raw::InstructionKind::WeakClone {
                    place: raw::PlaceId(1),
                    cleanup: raw::CleanupPlanId(2),
                },
            },
            raw::Instruction {
                result: Some(value(6, self.shared_string)),
                span,
                kind: raw::InstructionKind::SharedConstruct {
                    value: raw::ValueId(2),
                    cleanup: raw::CleanupPlanId(3),
                },
            },
        ];

        function.blocks[0].terminators[0].kind =
            raw::Terminator::Return { value: raw::ValueId(6), cleanup: raw::CleanupPlanId(4) };

        function.cleanup_plans = [
            vec![2, 1, 0],          // site 0 (SharedClone)
            vec![3, 2, 1, 0],       // site 1 (WeakDowngrade)
            vec![4, 3, 2, 1, 0],    // site 2 (WeakClone)
            vec![5, 4, 3, 2, 1, 0], // site 3 (SharedConstruct)
            vec![5, 4, 3, 1, 0],    // return cleanup (value 6 returned, 2 moved)
        ]
        .into_iter()
        .enumerate()
        .map(|(id, roots)| raw::CleanupPlan {
            id: raw::CleanupPlanId(u32::try_from(id).expect("cleanup plan id")),
            span,
            actions: roots
                .into_iter()
                .map(|id| raw::DropAction::DropPlace(raw::PlaceId(id)))
                .collect(),
        })
        .collect();

        raw
    }

    pub(super) fn seed_upgrade_branch(&self) -> raw::Program {
        let mut raw = program(&self.sources, &self.linear, &self.linux);
        let function = &mut raw.modules[0].functions[0];
        let span = function.span;
        let value = |id, ty| raw::ValueDefinition { id: raw::ValueId(id), ty, span };

        function.entry_export = None;
        function.parameters = vec![value(0, self.shared_string), value(1, self.weak_string)];
        function.result = self.shared_string;

        function.blocks = vec![
            raw::Block {
                id: raw::BlockId(0),
                parameters: vec![],
                instructions: vec![],
                terminators: vec![raw::SpannedTerminator {
                    span,
                    kind: raw::Terminator::WeakUpgradeBranch {
                        weak: raw::PlaceId(1),
                        success: raw::Edge { target: raw::BlockId(1), arguments: vec![] },
                        expired: raw::Edge { target: raw::BlockId(2), arguments: vec![] },
                        cleanup: raw::CleanupPlanId(0),
                    },
                }],
            },
            raw::Block {
                id: raw::BlockId(1),
                parameters: vec![value(2, self.shared_string)],
                instructions: vec![],
                terminators: vec![raw::SpannedTerminator {
                    span,
                    kind: raw::Terminator::Return {
                        value: raw::ValueId(2),
                        cleanup: raw::CleanupPlanId(1),
                    },
                }],
            },
            raw::Block {
                id: raw::BlockId(2),
                parameters: vec![],
                instructions: vec![],
                terminators: vec![raw::SpannedTerminator {
                    span,
                    kind: raw::Terminator::Return {
                        value: raw::ValueId(0),
                        cleanup: raw::CleanupPlanId(2),
                    },
                }],
            },
        ];

        function.places = vec![
            raw::Place {
                id: raw::PlaceId(0),
                ty: self.shared_string,
                span,
                kind: raw::PlaceKind::Parameter(0),
            },
            raw::Place {
                id: raw::PlaceId(1),
                ty: self.weak_string,
                span,
                kind: raw::PlaceKind::Parameter(1),
            },
            raw::Place {
                id: raw::PlaceId(2),
                ty: self.shared_string,
                span,
                kind: raw::PlaceKind::Temporary(raw::ValueId(2)),
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
                    raw::DropAction::DropPlace(raw::PlaceId(1)),
                    raw::DropAction::DropPlace(raw::PlaceId(0)),
                ],
            },
            raw::CleanupPlan {
                id: raw::CleanupPlanId(2),
                span,
                actions: vec![raw::DropAction::DropPlace(raw::PlaceId(1))],
            },
        ];

        raw
    }

    pub(super) fn verify(
        &self,
        raw: raw::Program,
    ) -> Result<crate::data_ownership_v1::VerifiedProgram, Vec<zryna_diagnostics::Diagnostic>> {
        verify(
            raw,
            &self.sources,
            self.sources.verify_file_id(0).expect("entry"),
            self.linear.clone(),
            self.linux.clone(),
        )
    }
}
