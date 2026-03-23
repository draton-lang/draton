use crate::error::OwnershipError;
use crate::typed_ast::{TypedFnDef, TypedItem, TypedProgram, TypedTypeBlock, TypedTypeMember};

/// Phase 2 stub for the inferred ownership checker.
pub struct OwnershipChecker;

impl OwnershipChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn check_program(&mut self, program: &mut TypedProgram) -> Vec<OwnershipError> {
        for item in &mut program.items {
            self.visit_item(item);
        }
        Vec::new()
    }

    fn visit_item(&mut self, item: &mut TypedItem) {
        match item {
            TypedItem::Fn(function)
            | TypedItem::PanicHandler(function)
            | TypedItem::OomHandler(function) => self.visit_fn(function),
            TypedItem::Class(class_def) => {
                for method in &mut class_def.methods {
                    self.visit_fn(method);
                }
                for type_block in &mut class_def.type_blocks {
                    self.visit_type_block(type_block);
                }
            }
            TypedItem::Interface(interface_def) => {
                for method in &mut interface_def.methods {
                    self.visit_fn(method);
                }
                for type_block in &mut interface_def.type_blocks {
                    self.visit_type_block(type_block);
                }
            }
            TypedItem::Extern(extern_block) => {
                for function in &mut extern_block.functions {
                    self.visit_fn(function);
                }
            }
            TypedItem::TypeBlock(type_block) => self.visit_type_block(type_block),
            TypedItem::Enum(_)
            | TypedItem::Error(_)
            | TypedItem::Const(_)
            | TypedItem::Import(_) => {}
        }
    }

    fn visit_type_block(&mut self, type_block: &mut TypedTypeBlock) {
        for member in &mut type_block.members {
            if let TypedTypeMember::Function(function) = member {
                self.visit_fn(function);
            }
        }
    }

    fn visit_fn(&mut self, function: &mut TypedFnDef) {
        // TODO(Phase 3): infer use-site effects, binding ownership states,
        // and function ownership summaries from the typed function body.
        let _ = function;
    }
}
