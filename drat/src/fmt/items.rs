use draton_ast::{
    ClassDef, ClassMember, ConstDef, EnumDef, ErrorDef, ExternBlock, FnDef, ImportDef, Item, Param,
    TypeBlock, TypeMember,
};

use super::Printer;

impl Printer {
    pub(crate) fn fmt_item(&mut self, item: &Item) {
        match item {
            Item::Fn(function) => self.fmt_fn(function),
            Item::Class(class_def) => self.fmt_class(class_def),
            Item::Interface(interface_def) => self.fmt_interface(interface_def),
            Item::Enum(enum_def) => self.fmt_enum(enum_def),
            Item::Error(error_def) => self.fmt_error(error_def),
            Item::Const(const_def) => self.fmt_const(const_def),
            Item::Import(import_def) => self.fmt_import(import_def),
            Item::Extern(extern_block) => self.fmt_extern(extern_block),
            Item::TypeBlock(type_block) => self.fmt_type_block(type_block),
            Item::PanicHandler(function) => {
                self.write("@panic_handler ");
                self.fmt_fn(function);
            }
            Item::OomHandler(function) => {
                self.write("@oom_handler ");
                self.fmt_fn(function);
            }
        }
    }

    pub(crate) fn fmt_fn(&mut self, function: &FnDef) {
        if function.is_pub {
            self.write("pub ");
        }
        self.write("fn ");
        self.write(&function.name);
        self.fmt_params(&function.params);
        if let Some(ret_type) = &function.ret_type {
            self.write(" -> ");
            self.fmt_type_expr(ret_type);
        }
        if let Some(body) = &function.body {
            self.write(" ");
            self.fmt_block(body);
        }
    }

    fn fmt_params(&mut self, params: &[Param]) {
        self.write("(");
        for (index, param) in params.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.write(&param.name);
            if let Some(type_hint) = &param.type_hint {
                self.write(": ");
                self.fmt_type_expr(type_hint);
            }
        }
        self.write(")");
    }

    fn fmt_class(&mut self, class_def: &ClassDef) {
        self.write("class ");
        self.write(&class_def.name);
        if !class_def.type_params.is_empty() {
            self.write("[");
            self.write(&class_def.type_params.join(", "));
            self.write("]");
        }
        if let Some(parent) = &class_def.extends {
            self.write(" extends ");
            self.write(parent);
        }
        if !class_def.implements.is_empty() {
            self.write(" implements ");
            self.write(&class_def.implements.join(", "));
        }
        self.write(" {");
        self.newline();
        self.push_indent();

        let mut first = true;
        let mut previous_field = false;
        for member in &class_def.members {
            if !first && previous_field && !matches!(member, ClassMember::Field(_)) {
                self.newline();
            }
            self.write_indent();
            match member {
                ClassMember::Field(field) => {
                    self.fmt_field(field);
                    previous_field = true;
                }
                ClassMember::Method(method) => {
                    self.fmt_fn(method);
                    previous_field = false;
                }
                ClassMember::Layer(layer) => {
                    self.write("layer ");
                    self.write(&layer.name);
                    self.write(" {");
                    self.newline();
                    self.push_indent();
                    for method in &layer.methods {
                        self.write_indent();
                        self.fmt_fn(method);
                        self.newline();
                    }
                    for type_block in &layer.type_blocks {
                        self.write_indent();
                        self.fmt_type_block(type_block);
                        self.newline();
                    }
                    self.pop_indent();
                    self.write_indent();
                    self.write("}");
                    previous_field = false;
                }
            }
            self.newline();
            first = false;
        }

        for type_block in &class_def.type_blocks {
            if !first {
                self.newline();
            }
            self.write_indent();
            self.fmt_type_block(type_block);
            self.newline();
            first = false;
        }

        self.pop_indent();
        self.write("}");
    }

    fn fmt_field(&mut self, field: &draton_ast::FieldDef) {
        if field.is_mut {
            self.write("let mut ");
        } else {
            self.write("let ");
        }
        self.write(&field.name);
        if let Some(type_hint) = &field.type_hint {
            self.write(": ");
            self.fmt_type_expr(type_hint);
        }
    }

    fn fmt_interface(&mut self, interface_def: &draton_ast::InterfaceDef) {
        self.write("interface ");
        self.write(&interface_def.name);
        self.write(" {");
        self.newline();
        self.push_indent();
        for method in &interface_def.methods {
            self.write_indent();
            self.fmt_fn(method);
            self.newline();
        }
        self.pop_indent();
        self.write("}");
    }

    fn fmt_enum(&mut self, enum_def: &EnumDef) {
        self.write("enum ");
        self.write(&enum_def.name);
        self.write(" {");
        self.newline();
        self.push_indent();
        for variant in &enum_def.variants {
            self.write_indent();
            self.write(variant);
            self.newline();
        }
        self.pop_indent();
        self.write("}");
    }

    fn fmt_error(&mut self, error_def: &ErrorDef) {
        self.write("error ");
        self.write(&error_def.name);
        self.write("(");
        for (index, field) in error_def.fields.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.write(&field.name);
            if let Some(type_hint) = &field.type_hint {
                self.write(": ");
                self.fmt_type_expr(type_hint);
            }
        }
        self.write(")");
    }

    fn fmt_const(&mut self, const_def: &ConstDef) {
        self.write("const ");
        self.write(&const_def.name);
        self.write(" = ");
        self.fmt_expr(&const_def.value);
    }

    fn fmt_import(&mut self, import_def: &ImportDef) {
        self.write("import { ");
        for (index, item) in import_def.items.iter().enumerate() {
            if index > 0 {
                self.write(", ");
            }
            self.write(&item.name);
            if let Some(alias) = &item.alias {
                self.write(" as ");
                self.write(alias);
            }
        }
        self.write(" }");
        if !import_def.module.is_empty() {
            self.write(" from ");
            self.write(&import_def.module.join("."));
        }
    }

    fn fmt_extern(&mut self, extern_block: &ExternBlock) {
        self.write("@extern ");
        self.write("\"");
        self.write(&extern_block.abi);
        self.write("\" {");
        self.newline();
        self.push_indent();
        for function in &extern_block.functions {
            self.write_indent();
            self.fmt_fn(function);
            self.newline();
        }
        self.pop_indent();
        self.write("}");
    }

    fn fmt_type_block(&mut self, type_block: &TypeBlock) {
        self.write("@type {");
        self.newline();
        self.push_indent();
        for member in &type_block.members {
            self.write_indent();
            match member {
                TypeMember::Binding {
                    name, type_expr, ..
                } => {
                    self.write(name);
                    self.write(": ");
                    self.fmt_type_expr(type_expr);
                }
                TypeMember::Function(function) => self.fmt_fn(function),
            }
            self.newline();
        }
        self.pop_indent();
        self.write("}");
    }
}
