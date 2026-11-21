use crate::ast::*;

/// Par de arquivos `.h` e `.cpp` C++17 gerados deterministicamente para Godot 4 (GDExtension ou Módulo SCons).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GodotClassOutput {
    pub class_name: String,
    pub header_filename: String,
    pub header_code: String,
    pub source_filename: String,
    pub source_code: String,
}

/// Bundle completo para plugin GDExtension C++17 (`rtsc --target godot`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GodotExtensionOutput {
    pub classes: Vec<GodotClassOutput>,
    pub register_types_h: String,
    pub register_types_cpp: String,
    pub sconstruct: String,
    pub gdextension_manifest_filename: String,
    pub gdextension_manifest: String,
}

/// Bundle completo para módulo customizado compilado dentro da árvore Godot 4 (`rtsc --target godot-module`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GodotModuleOutput {
    pub classes: Vec<GodotClassOutput>,
    pub register_types_h: String,
    pub register_types_cpp: String,
    pub scsub: String,
    pub config_py: String,
}

/// Transpilador para Godot Engine 4 (`rtsc --target godot` e `--target godot-module` — v2.3.0-rts).
/// Gera classes `GDCLASS`, bindings `_bind_methods` com `ClassDB`, propriedades `@gdexport` via `ADD_PROPERTY`,
/// `register_types.{h,cpp}` (`GDREGISTER_CLASS` / `ClassDB::register_class`) e scripts `SConstruct` / `SCsub` / `.gdextension`.
pub struct GodotCodegen;

impl GodotCodegen {
    /// Emite um projeto completo GDExtension C++17 (`godot-cpp`) pronto para compilação via `scons`.
    pub fn emit_godot_extension(module: &Module) -> GodotExtensionOutput {
        let mod_snake = Self::module_ident(module, "rustts_godot");
        let (classes, class_names) = Self::emit_godot_classes(module, false);
        let guard = format!("{}_REGISTER_TYPES_H", mod_snake.to_ascii_uppercase());

        let mut reg_h = String::new();
        reg_h.push_str("// Generated deterministically by rtsc 2.3.0-rts --target=godot\n");
        reg_h.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
        reg_h.push_str("#include <godot_cpp/core/class_db.hpp>\n\n");
        reg_h.push_str("using namespace godot;\n\n");
        reg_h.push_str(&format!(
            "void initialize_{}_module(ModuleInitializationLevel p_level);\n",
            mod_snake
        ));
        reg_h.push_str(&format!(
            "void uninitialize_{}_module(ModuleInitializationLevel p_level);\n\n",
            mod_snake
        ));
        reg_h.push_str(&format!("#endif // {}\n", guard));

        let mut reg_cpp = String::new();
        reg_cpp.push_str("// Generated deterministically by rtsc 2.3.0-rts --target=godot\n");
        reg_cpp.push_str("#include \"register_types.h\"\n\n");
        for c in &classes {
            reg_cpp.push_str(&format!("#include \"{}\"\n", c.header_filename));
        }
        reg_cpp.push_str("\n#include <gdextension_interface.h>\n");
        reg_cpp.push_str("#include <godot_cpp/core/defs.hpp>\n");
        reg_cpp.push_str("#include <godot_cpp/godot.hpp>\n\n");
        reg_cpp.push_str("using namespace godot;\n\n");
        reg_cpp.push_str(&format!(
            "void initialize_{}_module(ModuleInitializationLevel p_level) {{\n",
            mod_snake
        ));
        reg_cpp.push_str("    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {\n        return;\n    }\n");
        for name in &class_names {
            reg_cpp.push_str(&format!("    GDREGISTER_CLASS({});\n", name));
        }
        reg_cpp.push_str("}\n\n");
        reg_cpp.push_str(&format!(
            "void uninitialize_{}_module(ModuleInitializationLevel p_level) {{\n",
            mod_snake
        ));
        reg_cpp.push_str("    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {\n        return;\n    }\n}\n\n");
        reg_cpp.push_str("extern \"C\" {\n");
        reg_cpp.push_str(&format!(
            "GDExtensionBool GDE_EXPORT {}_library_init(\n",
            mod_snake
        ));
        reg_cpp.push_str("    GDExtensionInterfaceGetProcAddress p_get_proc_address,\n");
        reg_cpp.push_str("    const GDExtensionClassLibraryPtr p_library,\n");
        reg_cpp.push_str("    GDExtensionInitialization *r_initialization\n) {\n");
        reg_cpp.push_str(
            "    godot::GDExtensionBinding::InitObject init_obj(p_get_proc_address, p_library, r_initialization);\n\n",
        );
        reg_cpp.push_str(&format!(
            "    init_obj.register_initializer(initialize_{}_module);\n",
            mod_snake
        ));
        reg_cpp.push_str(&format!(
            "    init_obj.register_terminator(uninitialize_{}_module);\n",
            mod_snake
        ));
        reg_cpp.push_str(
            "    init_obj.set_minimum_library_initialization_level(MODULE_INITIALIZATION_LEVEL_SCENE);\n\n",
        );
        reg_cpp.push_str("    return init_obj.init();\n}\n}\n");

        let sconstruct = format!(
            "#!/usr/bin/env python\n\
             # Generated deterministically by rtsc 2.3.0-rts --target=godot\n\
             import os\n\
             import sys\n\n\
             env = SConscript(\"godot-cpp/SConstruct\")\n\
             env.Append(CPPPATH=[\"src/\"])\n\
             sources = Glob(\"src/*.cpp\")\n\n\
             if env[\"platform\"] == \"macos\":\n    \
                 library = env.SharedLibrary(\n        \
                     \"bin/lib{mod}.{{}}.{{}}.framework/lib{mod}.{{}}.{{}}\".format(\n            \
                         env[\"platform\"], env[\"target\"], env[\"platform\"], env[\"target\"]\n        \
                     ),\n        \
                     source=sources,\n    \
                 )\n\
             else:\n    \
                 library = env.SharedLibrary(\n        \
                     \"bin/lib{mod}{{}}{{}}\".format(env[\"suffix\"], env[\"SHLIBSUFFIX\"]),\n        \
                     source=sources,\n    \
                 )\n\n\
             Default(library)\n",
            mod = mod_snake
        );

        let manifest_filename = format!("{}.gdextension", mod_snake);
        let manifest = format!(
            "[configuration]\n\
             entry_symbol = \"{mod}_library_init\"\n\
             compatibility_minimum = \"4.2\"\n\
             reloadable = true\n\n\
             [libraries]\n\
             macos.debug = \"res://bin/lib{mod}.macos.template_debug.framework\"\n\
             macos.release = \"res://bin/lib{mod}.macos.template_release.framework\"\n\
             windows.debug.x86_64 = \"res://bin/lib{mod}.windows.template_debug.x86_64.dll\"\n\
             windows.release.x86_64 = \"res://bin/lib{mod}.windows.template_release.x86_64.dll\"\n\
             linux.debug.x86_64 = \"res://bin/lib{mod}.linux.template_debug.x86_64.so\"\n\
             linux.release.x86_64 = \"res://bin/lib{mod}.linux.template_release.x86_64.so\"\n",
            mod = mod_snake
        );

        GodotExtensionOutput {
            classes,
            register_types_h: reg_h,
            register_types_cpp: reg_cpp,
            sconstruct,
            gdextension_manifest_filename: manifest_filename,
            gdextension_manifest: manifest,
        }
    }

    /// Emite um módulo customizado C++17 para compilação direta na fonte da Godot 4 via SCons (`custom_modules`).
    pub fn emit_godot_module(module: &Module) -> GodotModuleOutput {
        let mod_snake = Self::module_ident(module, "rustts_module");
        let (classes, class_names) = Self::emit_godot_classes(module, true);
        let guard = format!("{}_REGISTER_TYPES_H", mod_snake.to_ascii_uppercase());

        let mut reg_h = String::new();
        reg_h.push_str("// Generated deterministically by rtsc 2.3.0-rts --target=godot-module\n");
        reg_h.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
        reg_h.push_str("#include \"modules/register_module_types.h\"\n\n");
        reg_h.push_str(&format!(
            "void initialize_{}_module(ModuleInitializationLevel p_level);\n",
            mod_snake
        ));
        reg_h.push_str(&format!(
            "void uninitialize_{}_module(ModuleInitializationLevel p_level);\n\n",
            mod_snake
        ));
        reg_h.push_str(&format!("#endif // {}\n", guard));

        let mut reg_cpp = String::new();
        reg_cpp.push_str("// Generated deterministically by rtsc 2.3.0-rts --target=godot-module\n");
        reg_cpp.push_str("#include \"register_types.h\"\n\n");
        reg_cpp.push_str("#include \"core/object/class_db.h\"\n");
        for c in &classes {
            reg_cpp.push_str(&format!("#include \"{}\"\n", c.header_filename));
        }
        reg_cpp.push_str(&format!(
            "\nvoid initialize_{}_module(ModuleInitializationLevel p_level) {{\n",
            mod_snake
        ));
        reg_cpp.push_str("    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {\n        return;\n    }\n");
        for name in &class_names {
            reg_cpp.push_str(&format!("    ClassDB::register_class<{}>();\n", name));
        }
        reg_cpp.push_str("}\n\n");
        reg_cpp.push_str(&format!(
            "void uninitialize_{}_module(ModuleInitializationLevel p_level) {{\n",
            mod_snake
        ));
        reg_cpp.push_str("    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {\n        return;\n    }\n}\n");

        let scsub = "#!/usr/bin/env python\n\
                     # Generated deterministically by rtsc 2.3.0-rts --target=godot-module\n\
                     Import(\"env\")\n\n\
                     module_env = env.Clone()\n\
                     module_env.add_source_files(env.modules_sources, \"*.cpp\")\n"
            .to_string();

        let config_py = "# Generated deterministically by rtsc 2.3.0-rts --target=godot-module\n\
                         def can_build(env, platform):\n    \
                             return True\n\n\
                         def configure(env):\n    \
                             pass\n"
            .to_string();

        GodotModuleOutput {
            classes,
            register_types_h: reg_h,
            register_types_cpp: reg_cpp,
            scsub,
            config_py,
        }
    }

    fn emit_godot_classes(module: &Module, is_module: bool) -> (Vec<GodotClassOutput>, Vec<String>) {
        let mut classes = Vec::new();
        let mut class_names = Vec::new();
        let target_label = if is_module { "godot-module" } else { "godot" };

        for item in &module.items {
            if let Item::Struct(s) = item {
                let node_attr = s.attributes.iter().find_map(|a| match a {
                    Attribute::GodotNode {
                        base_class,
                        category,
                    } => Some((base_class.clone(), category.clone())),
                    _ => None,
                });

                if let Some((base_class, category)) = node_attr {
                    let class_name = s.name.clone();
                    let snake = Self::to_snake_case(&class_name);
                    let header_filename = format!("{}.h", snake);
                    let source_filename = format!("{}.cpp", snake);
                    let guard = format!("{}_H", snake.to_ascii_uppercase());

                    let methods: Vec<&FnDecl> = module
                        .items
                        .iter()
                        .filter_map(|it| match it {
                            Item::Function(f) => {
                                let matches_target = f.attributes.iter().any(|attr| {
                                    matches!(
                                        attr,
                                        Attribute::GodotImpl { target_struct }
                                            | Attribute::UnrealImpl { target_struct }
                                            if target_struct == &s.name
                                    )
                                });
                                if matches_target {
                                    Some(f)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect();

                    let mut h = String::new();
                    h.push_str(&format!(
                        "// Generated deterministically by rtsc 2.3.0-rts --target={}\n",
                        target_label
                    ));
                    h.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
                    if is_module {
                        h.push_str(&format!(
                            "#include \"{}\"\n",
                            Self::godot_module_base_include(&base_class)
                        ));
                        h.push_str("#include \"core/object/class_db.h\"\n");
                        h.push_str("#include \"core/variant/variant.h\"\n\n");
                    } else {
                        h.push_str(&format!(
                            "#include <godot_cpp/classes/{}.hpp>\n",
                            Self::to_godot_cpp_header_name(&base_class)
                        ));
                        h.push_str("#include <godot_cpp/core/class_db.hpp>\n");
                        h.push_str("#include <godot_cpp/variant/utility_functions.hpp>\n\n");
                        h.push_str("namespace godot {\n\n");
                    }

                    h.push_str(&format!("// Category: {}\n", category));
                    h.push_str(&format!(
                        "class {} : public {} {{\n    GDCLASS({}, {});\n\n",
                        class_name, base_class, class_name, base_class
                    ));

                    for field in &s.fields {
                        h.push_str(&format!(
                            "    {} {};\n",
                            Self::map_godot_type(&field.ty),
                            field.name
                        ));
                    }
                    if !s.fields.is_empty() {
                        h.push('\n');
                    }

                    h.push_str("protected:\n    static void _bind_methods();\n\npublic:\n");
                    h.push_str(&format!("    {}();\n    ~{}();\n\n", class_name, class_name));

                    let mut exported_fields = Vec::new();
                    for field in &s.fields {
                        let export_attr = field.attributes.iter().find_map(|a| match a {
                            Attribute::GdExport { hint, hint_string } => {
                                Some((hint.clone(), hint_string.clone()))
                            }
                            _ => None,
                        });
                        if let Some((hint, hint_string)) = export_attr {
                            let c_ty = Self::map_godot_type(&field.ty);
                            let param_ty = Self::map_godot_param_type(&field.ty);
                            h.push_str(&format!(
                                "    void set_{}({} p_value);\n    {} get_{}() const;\n\n",
                                field.name, param_ty, c_ty, field.name
                            ));
                            exported_fields.push((field, hint, hint_string));
                        }
                    }

                    for m in &methods {
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(Self::map_godot_type)
                            .unwrap_or_else(|| "void".to_string());
                        let is_const = Self::is_readonly_self_method(m);
                        let const_qual = if is_const { " const" } else { "" };
                        let ov_qual = if Self::is_godot_virtual_override(&m.name) {
                            " override"
                        } else {
                            ""
                        };
                        let params_cpp: Vec<String> = m
                            .params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{} {}", Self::map_godot_type(&p.ty), p.name))
                            .collect();
                        h.push_str(&format!(
                            "    {} {}({}){}{};\n",
                            ret_ty,
                            m.name,
                            params_cpp.join(", "),
                            const_qual,
                            ov_qual
                        ));
                    }
                    h.push_str("};\n");
                    if !is_module {
                        h.push_str("\n} // namespace godot\n");
                    }
                    h.push_str(&format!("\n#endif // {}\n", guard));

                    let mut cpp = String::new();
                    cpp.push_str(&format!(
                        "// Generated deterministically by rtsc 2.3.0-rts --target={}\n",
                        target_label
                    ));
                    cpp.push_str(&format!("#include \"{}\"\n", header_filename));
                    if is_module {
                        cpp.push_str("#include \"core/math/math_funcs.h\"\n");
                        cpp.push_str("#include \"core/variant/utility_functions.h\"\n\n");
                    } else {
                        cpp.push_str("#include <godot_cpp/core/math.hpp>\n\n");
                        cpp.push_str("namespace godot {\n\n");
                    }

                    cpp.push_str(&format!("{}::{}() {{\n", class_name, class_name));
                    for field in &s.fields {
                        if let Some(init_lit) = Self::default_scalar_literal(&field.ty) {
                            cpp.push_str(&format!("    {} = {};\n", field.name, init_lit));
                        }
                    }
                    cpp.push_str("}\n\n");
                    cpp.push_str(&format!("{}::~{}() {{}}\n\n", class_name, class_name));

                    cpp.push_str(&format!("void {}::_bind_methods() {{\n", class_name));
                    for m in &methods {
                        if Self::is_godot_virtual_override(&m.name) {
                            continue;
                        }
                        let arg_names: Vec<String> = m
                            .params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("\"{}\"", p.name))
                            .collect();
                        if arg_names.is_empty() {
                            cpp.push_str(&format!(
                                "    ClassDB::bind_method(D_METHOD(\"{}\"), &{}::{});\n",
                                m.name, class_name, m.name
                            ));
                        } else {
                            cpp.push_str(&format!(
                                "    ClassDB::bind_method(D_METHOD(\"{}\", {}), &{}::{});\n",
                                m.name,
                                arg_names.join(", "),
                                class_name,
                                m.name
                            ));
                        }
                    }
                    for (field, hint, hint_string) in &exported_fields {
                        cpp.push_str(&format!(
                            "    ClassDB::bind_method(D_METHOD(\"get_{}\"), &{}::get_{});\n",
                            field.name, class_name, field.name
                        ));
                        cpp.push_str(&format!(
                            "    ClassDB::bind_method(D_METHOD(\"set_{}\", \"p_value\"), &{}::set_{});\n",
                            field.name, class_name, field.name
                        ));
                        let v_ty = Self::map_godot_variant_type(&field.ty);
                        if let Some(h_name) = hint {
                            let h_const = Self::map_godot_property_hint(h_name);
                            let h_str = hint_string.as_deref().unwrap_or("");
                            cpp.push_str(&format!(
                                "    ADD_PROPERTY(PropertyInfo({}, \"{}\", {}, \"{}\"), \"set_{}\", \"get_{}\");\n",
                                v_ty, field.name, h_const, h_str, field.name, field.name
                            ));
                        } else {
                            cpp.push_str(&format!(
                                "    ADD_PROPERTY(PropertyInfo({}, \"{}\"), \"set_{}\", \"get_{}\");\n",
                                v_ty, field.name, field.name, field.name
                            ));
                        }
                    }
                    cpp.push_str("}\n\n");

                    for (field, _, _) in &exported_fields {
                        let c_ty = Self::map_godot_type(&field.ty);
                        let param_ty = Self::map_godot_param_type(&field.ty);
                        cpp.push_str(&format!(
                            "void {}::set_{}({} p_value) {{\n    {} = p_value;\n}}\n\n",
                            class_name, field.name, param_ty, field.name
                        ));
                        cpp.push_str(&format!(
                            "{} {}::get_{}() const {{\n    return {};\n}}\n\n",
                            c_ty, class_name, field.name, field.name
                        ));
                    }

                    for m in &methods {
                        let ret_ty = m
                            .return_type
                            .as_ref()
                            .map(Self::map_godot_type)
                            .unwrap_or_else(|| "void".to_string());
                        let is_const = Self::is_readonly_self_method(m);
                        let const_qual = if is_const { " const" } else { "" };
                        let params_cpp: Vec<String> = m
                            .params
                            .iter()
                            .filter(|p| p.name != "self")
                            .map(|p| format!("{} {}", Self::map_godot_type(&p.ty), p.name))
                            .collect();
                        cpp.push_str(&format!(
                            "{} {}::{}({}){} {{\n",
                            ret_ty,
                            class_name,
                            m.name,
                            params_cpp.join(", "),
                            const_qual
                        ));
                        cpp.push_str("    // Capability-checked deterministic body lowered from RustTS AST\n");
                        for stmt in &m.body {
                            Self::lower_godot_stmt(&mut cpp, stmt, 1);
                        }
                        cpp.push_str("}\n\n");
                    }

                    if !is_module {
                        cpp.push_str("} // namespace godot\n");
                    }

                    class_names.push(class_name.clone());
                    classes.push(GodotClassOutput {
                        class_name,
                        header_filename,
                        header_code: h,
                        source_filename,
                        source_code: cpp,
                    });
                }
            }
        }

        // Fallback para módulos utilitários sem `@godot_node`: expõe `pub fn` em `RefCounted` via `bind_static_method`
        if classes.is_empty() {
            let class_name = format!("{}Extension", Self::module_pascal_name(module));
            let snake = Self::to_snake_case(&class_name);
            let header_filename = format!("{}.h", snake);
            let source_filename = format!("{}.cpp", snake);
            let guard = format!("{}_H", snake.to_ascii_uppercase());

            let pub_fns: Vec<&FnDecl> = module
                .items
                .iter()
                .filter_map(|it| match it {
                    Item::Function(f) if f.is_pub => Some(f),
                    _ => None,
                })
                .collect();

            let mut h = String::new();
            h.push_str(&format!(
                "// Generated deterministically by rtsc 2.3.0-rts --target={}\n",
                target_label
            ));
            h.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
            if is_module {
                h.push_str("#include \"core/object/ref_counted.h\"\n");
                h.push_str("#include \"core/object/class_db.h\"\n\n");
            } else {
                h.push_str("#include <godot_cpp/classes/ref_counted.hpp>\n");
                h.push_str("#include <godot_cpp/core/class_db.hpp>\n\n");
                h.push_str("namespace godot {\n\n");
            }
            h.push_str(&format!(
                "class {} : public RefCounted {{\n    GDCLASS({}, RefCounted);\n\nprotected:\n    static void _bind_methods();\n\npublic:\n",
                class_name, class_name
            ));
            for f in &pub_fns {
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(Self::map_godot_type)
                    .unwrap_or_else(|| "void".to_string());
                let params_cpp: Vec<String> = f
                    .params
                    .iter()
                    .filter(|p| !p.is_cap)
                    .map(|p| format!("{} {}", Self::map_godot_type(&p.ty), p.name))
                    .collect();
                h.push_str(&format!(
                    "    static {} {}({});\n",
                    ret_ty,
                    f.name,
                    params_cpp.join(", ")
                ));
            }
            h.push_str("};\n");
            if !is_module {
                h.push_str("\n} // namespace godot\n");
            }
            h.push_str(&format!("\n#endif // {}\n", guard));

            let mut cpp = String::new();
            cpp.push_str(&format!(
                "// Generated deterministically by rtsc 2.3.0-rts --target={}\n",
                target_label
            ));
            cpp.push_str(&format!("#include \"{}\"\n", header_filename));
            if is_module {
                cpp.push_str("#include \"core/math/math_funcs.h\"\n");
                cpp.push_str("#include \"core/variant/utility_functions.h\"\n\n");
            } else {
                cpp.push_str("#include <godot_cpp/core/math.hpp>\n");
                cpp.push_str("#include <godot_cpp/variant/utility_functions.hpp>\n\n");
                cpp.push_str("namespace godot {\n\n");
            }
            cpp.push_str(&format!("void {}::_bind_methods() {{\n", class_name));
            for f in &pub_fns {
                let arg_names: Vec<String> = f
                    .params
                    .iter()
                    .filter(|p| !p.is_cap)
                    .map(|p| format!("\"{}\"", p.name))
                    .collect();
                if arg_names.is_empty() {
                    cpp.push_str(&format!(
                        "    ClassDB::bind_static_method(\"{}\", D_METHOD(\"{}\"), &{}::{});\n",
                        class_name, f.name, class_name, f.name
                    ));
                } else {
                    cpp.push_str(&format!(
                        "    ClassDB::bind_static_method(\"{}\", D_METHOD(\"{}\", {}), &{}::{});\n",
                        class_name,
                        f.name,
                        arg_names.join(", "),
                        class_name,
                        f.name
                    ));
                }
            }
            cpp.push_str("}\n\n");
            for f in &pub_fns {
                let ret_ty = f
                    .return_type
                    .as_ref()
                    .map(Self::map_godot_type)
                    .unwrap_or_else(|| "void".to_string());
                let params_cpp: Vec<String> = f
                    .params
                    .iter()
                    .filter(|p| !p.is_cap)
                    .map(|p| format!("{} {}", Self::map_godot_type(&p.ty), p.name))
                    .collect();
                cpp.push_str(&format!(
                    "{} {}::{}({}) {{\n",
                    ret_ty,
                    class_name,
                    f.name,
                    params_cpp.join(", ")
                ));
                for stmt in &f.body {
                    Self::lower_godot_stmt(&mut cpp, stmt, 1);
                }
                cpp.push_str("}\n\n");
            }
            if !is_module {
                cpp.push_str("} // namespace godot\n");
            }

            class_names.push(class_name.clone());
            classes.push(GodotClassOutput {
                class_name,
                header_filename,
                header_code: h,
                source_filename,
                source_code: cpp,
            });
        }

        (classes, class_names)
    }

    fn is_readonly_self_method(m: &FnDecl) -> bool {
        for p in &m.params {
            if p.name == "self" {
                if let Some(cap) = &p.capability {
                    if cap.kind == CapabilityKind::MutResource {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn is_godot_virtual_override(name: &str) -> bool {
        matches!(name, "_ready" | "_process" | "_physics_process")
    }

    fn module_ident(module: &Module, fallback: &str) -> String {
        if module.path.is_empty() {
            fallback.to_string()
        } else {
            module.path.join("_")
        }
    }

    fn module_pascal_name(module: &Module) -> String {
        if module.path.is_empty() {
            return "Rustts".to_string();
        }
        let mut out = String::new();
        for seg in &module.path {
            for part in seg.split('_').filter(|s| !s.is_empty()) {
                let mut ch = part.chars();
                if let Some(first) = ch.next() {
                    out.push(first.to_ascii_uppercase());
                    out.extend(ch);
                }
            }
        }
        if out.is_empty() {
            "Rustts".to_string()
        } else {
            out
        }
    }

    fn to_snake_case(name: &str) -> String {
        let mut out = String::new();
        let mut prev_lower_or_digit = false;
        for c in name.chars() {
            if c.is_ascii_uppercase() {
                if prev_lower_or_digit && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower_or_digit = false;
            } else {
                out.push(c);
                prev_lower_or_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
            }
        }
        out
    }

    fn to_godot_cpp_header_name(base_class: &str) -> String {
        match base_class {
            "Node" => "node".to_string(),
            "Node2D" => "node2d".to_string(),
            "Node3D" => "node3d".to_string(),
            "CharacterBody2D" => "character_body2d".to_string(),
            "CharacterBody3D" => "character_body3d".to_string(),
            "RigidBody2D" => "rigid_body2d".to_string(),
            "RigidBody3D" => "rigid_body3d".to_string(),
            "Area2D" => "area2d".to_string(),
            "Area3D" => "area3d".to_string(),
            "Control" => "control".to_string(),
            "RefCounted" => "ref_counted".to_string(),
            "Resource" => "resource".to_string(),
            other => Self::to_snake_case(other),
        }
    }

    fn godot_module_base_include(base_class: &str) -> &'static str {
        match base_class {
            "RefCounted" => "core/object/ref_counted.h",
            "Object" => "core/object/object.h",
            "Resource" => "core/io/resource.h",
            "Node" => "scene/main/node.h",
            "Node2D" => "scene/2d/node_2d.h",
            "Node3D" => "scene/3d/node_3d.h",
            "CharacterBody2D" => "scene/2d/physics/character_body_2d.h",
            "CharacterBody3D" => "scene/3d/physics/character_body_3d.h",
            "RigidBody2D" => "scene/2d/physics/rigid_body_2d.h",
            "RigidBody3D" => "scene/3d/physics/rigid_body_3d.h",
            "Area2D" => "scene/2d/physics/area_2d.h",
            "Area3D" => "scene/3d/physics/area_3d.h",
            "Control" => "scene/gui/control.h",
            _ => "scene/main/node.h",
        }
    }

    fn lower_godot_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
        let pad = "    ".repeat(indent);
        match stmt {
            Stmt::Let {
                name,
                is_mut,
                ty,
                value,
                ..
            } => {
                let const_kw = if *is_mut { "" } else { "const " };
                let cpp_ty = ty
                    .as_ref()
                    .map(Self::map_godot_type)
                    .unwrap_or_else(|| "auto".to_string());
                let val_cpp = Self::lower_godot_expr(value);
                out.push_str(&format!("{}{}{} {} = {};\n", pad, const_kw, cpp_ty, name, val_cpp));
            }
            Stmt::Assign { target, value, .. } => {
                let lhs = Self::lower_godot_expr(target);
                let rhs = Self::lower_godot_expr(value);
                out.push_str(&format!("{}{} = {};\n", pad, lhs, rhs));
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond = Self::lower_godot_expr(condition);
                out.push_str(&format!("{}if ({}) {{\n", pad, cond));
                for s in then_branch {
                    Self::lower_godot_stmt(out, s, indent + 1);
                }
                if let Some(eb) = else_branch {
                    out.push_str(&format!("{}}} else {{\n", pad));
                    for s in eb {
                        Self::lower_godot_stmt(out, s, indent + 1);
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Match { expr, arms, .. } => {
                let scrutinee = Self::lower_godot_expr(expr);
                out.push_str(&format!("{}switch ({}) {{\n", pad, scrutinee));
                for arm in arms {
                    match &arm.pattern {
                        MatchPattern::Variant {
                            enum_name, variant, ..
                        } => {
                            let case_lbl = match enum_name {
                                Some(en) => format!("{}::{}", en, variant),
                                None => variant.clone(),
                            };
                            out.push_str(&format!("{}case {}: {{\n", pad, case_lbl));
                            for s in &arm.body {
                                Self::lower_godot_stmt(out, s, indent + 1);
                            }
                            out.push_str(&format!("{}    break;\n{}}}\n", pad, pad));
                        }
                        MatchPattern::Wildcard => {
                            out.push_str(&format!("{}default: {{\n", pad));
                            for s in &arm.body {
                                Self::lower_godot_stmt(out, s, indent + 1);
                            }
                            out.push_str(&format!("{}    break;\n{}}}\n", pad, pad));
                        }
                    }
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::While {
                condition, body, ..
            } => {
                let cond = Self::lower_godot_expr(condition);
                out.push_str(&format!("{}while ({}) {{\n", pad, cond));
                for s in body {
                    Self::lower_godot_stmt(out, s, indent + 1);
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Break { .. } => out.push_str(&format!("{}break;\n", pad)),
            Stmt::Continue { .. } => out.push_str(&format!("{}continue;\n", pad)),
            Stmt::ForRange {
                iterator,
                start,
                end,
                body,
                ..
            } => {
                let s_cpp = Self::lower_godot_expr(start);
                let e_cpp = Self::lower_godot_expr(end);
                out.push_str(&format!(
                    "{}for (int32_t {} = {}; {} < {}; ++{}) {{\n",
                    pad, iterator, s_cpp, iterator, e_cpp, iterator
                ));
                for s in body {
                    Self::lower_godot_stmt(out, s, indent + 1);
                }
                out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Return { value, .. } => match value {
                Some(v) => {
                    out.push_str(&format!("{}return {};\n", pad, Self::lower_godot_expr(v)));
                }
                None => out.push_str(&format!("{}return;\n", pad)),
            },
            Stmt::ExprStmt { expr, .. } => {
                out.push_str(&format!("{}{};\n", pad, Self::lower_godot_expr(expr)));
            }
        }
    }

    fn lower_godot_expr(expr: &Expr) -> String {
        match expr {
            Expr::IntLit(v, _) => format!("{}", v),
            Expr::FloatLit(v, _) => {
                let s = format!("{}", v);
                if s.contains('.') {
                    format!("{}f", s)
                } else {
                    format!("{}.0f", s)
                }
            }
            Expr::BoolLit(b, _) => b.to_string(),
            Expr::StringLit(s, _) => format!("String(\"{}\")", s),
            Expr::Ident(name, _) => {
                if name == "self" {
                    "this".to_string()
                } else {
                    name.clone()
                }
            }
            Expr::Unary { op, expr: inner, .. } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                    UnaryOp::BitNot => "~",
                };
                format!("({}{})", op_str, Self::lower_godot_expr(inner))
            }
            Expr::FieldAccess {
                receiver, field, ..
            } => {
                if let Expr::Ident(rname, _) = receiver.as_ref() {
                    if rname == "self" {
                        return format!("this->{}", field);
                    }
                }
                format!("{}.{}", Self::lower_godot_expr(receiver), field)
            }
            Expr::StaticCall {
                namespace,
                func,
                args,
                ..
            } => {
                let args_cpp: Vec<String> = args.iter().map(Self::lower_godot_expr).collect();
                if namespace == "math" || namespace == "core.math" {
                    let gd_fn = match func.as_str() {
                        "clamp" => "Math::clamp",
                        "min" => "MIN",
                        "max" => "MAX",
                        "abs" => "Math::abs",
                        "sqrt" => "Math::sqrt",
                        "lerp" => "Math::lerp",
                        "sin" => "Math::sin",
                        "cos" => "Math::cos",
                        "tan" => "Math::tan",
                        "floor" => "Math::floor",
                        "pow" => "Math::pow",
                        other => other,
                    };
                    format!("{}({})", gd_fn, args_cpp.join(", "))
                } else if namespace == "io" || namespace == "core.io" {
                    format!("UtilityFunctions::print({})", args_cpp.join(", "))
                } else {
                    format!("{}::{}({})", namespace, func, args_cpp.join(", "))
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let rec = Self::lower_godot_expr(receiver);
                let args_cpp: Vec<String> = args.iter().map(Self::lower_godot_expr).collect();
                if rec == "this" {
                    format!("this->{}({})", method, args_cpp.join(", "))
                } else {
                    format!("{}.{}({})", rec, method, args_cpp.join(", "))
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = Self::lower_godot_expr(left);
                let r = Self::lower_godot_expr(right);
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Rem => "%",
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    BinaryOp::BitXor => "^",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Shr => ">>",
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                };
                format!("({} {} {})", l, op_str, r)
            }
            Expr::Cast {
                expr: inner,
                target_type,
                ..
            } => {
                format!(
                    "static_cast<{}>({})",
                    Self::map_godot_type(target_type),
                    Self::lower_godot_expr(inner)
                )
            }
            Expr::Index {
                receiver, index, ..
            } => {
                format!(
                    "{}[{}]",
                    Self::lower_godot_expr(receiver),
                    Self::lower_godot_expr(index)
                )
            }
            Expr::StructInit {
                struct_name,
                fields,
                ..
            } => {
                let vals: Vec<String> = fields
                    .iter()
                    .map(|(_, v)| Self::lower_godot_expr(v))
                    .collect();
                format!("{}({})", struct_name, vals.join(", "))
            }
            Expr::EnumConstructor {
                enum_name, variant, ..
            } => match enum_name {
                Some(en) => format!("{}::{}", en, variant),
                None => variant.clone(),
            },
            Expr::TryOp { expr: inner, .. } | Expr::ErrorMapClosure { body: inner, .. } => {
                Self::lower_godot_expr(inner)
            }
            Expr::ArrayLit { elements, .. } => {
                let elems: Vec<String> = elements.iter().map(Self::lower_godot_expr).collect();
                format!("{{ {} }}", elems.join(", "))
            }
            Expr::IfExpr {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                format!(
                    "(({}) ? ({}) : ({}))",
                    Self::lower_godot_expr(cond),
                    Self::lower_godot_expr(then_branch),
                    Self::lower_godot_expr(else_branch)
                )
            }
        }
    }

    fn default_scalar_literal(ty: &TypeExpr) -> Option<&'static str> {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::F32 => Some("0.0f"),
                PrimitiveType::F64 => Some("0.0"),
                PrimitiveType::Bool => Some("false"),
                PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::Usize
                | PrimitiveType::Fix32 => Some("0"),
                _ => None,
            },
            _ => None,
        }
    }

    fn map_godot_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::F32 => "float".to_string(),
                PrimitiveType::F64 => "double".to_string(),
                PrimitiveType::I8 => "int8_t".to_string(),
                PrimitiveType::I16 => "int16_t".to_string(),
                PrimitiveType::I32 | PrimitiveType::Fix32 => "int32_t".to_string(),
                PrimitiveType::I64 => "int64_t".to_string(),
                PrimitiveType::U8 => "uint8_t".to_string(),
                PrimitiveType::U16 => "uint16_t".to_string(),
                PrimitiveType::U32 => "uint32_t".to_string(),
                PrimitiveType::U64 | PrimitiveType::Usize => "uint64_t".to_string(),
                PrimitiveType::Bool => "bool".to_string(),
                PrimitiveType::Str
                | PrimitiveType::Str64
                | PrimitiveType::Str128
                | PrimitiveType::Str256 => "String".to_string(),
            },
            TypeExpr::Math(m) => match m {
                MathType::Vec2 => "Vector2".to_string(),
                MathType::Vec3 => "Vector3".to_string(),
                MathType::Vec4 => "Vector4".to_string(),
                MathType::Quat => "Quaternion".to_string(),
                MathType::Mat3 => "Basis".to_string(),
                MathType::Mat4 => "Transform3D".to_string(),
            },
            TypeExpr::Named { name, .. } => name.clone(),
            _ => "void*".to_string(),
        }
    }

    fn map_godot_param_type(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Primitive(
                PrimitiveType::Str
                | PrimitiveType::Str64
                | PrimitiveType::Str128
                | PrimitiveType::Str256,
            ) => "const String &".to_string(),
            TypeExpr::Math(m) => format!(
                "const {} &",
                Self::map_godot_type(&TypeExpr::Math(m.clone()))
            ),
            other => Self::map_godot_type(other),
        }
    }

    fn map_godot_variant_type(ty: &TypeExpr) -> &'static str {
        match ty {
            TypeExpr::Primitive(p) => match p {
                PrimitiveType::F32 | PrimitiveType::F64 => "Variant::FLOAT",
                PrimitiveType::Bool => "Variant::BOOL",
                PrimitiveType::Str
                | PrimitiveType::Str64
                | PrimitiveType::Str128
                | PrimitiveType::Str256 => "Variant::STRING",
                _ => "Variant::INT",
            },
            TypeExpr::Math(m) => match m {
                MathType::Vec2 => "Variant::VECTOR2",
                MathType::Vec3 => "Variant::VECTOR3",
                MathType::Vec4 => "Variant::VECTOR4",
                MathType::Quat => "Variant::QUATERNION",
                MathType::Mat3 => "Variant::BASIS",
                MathType::Mat4 => "Variant::TRANSFORM3D",
            },
            _ => "Variant::NIL",
        }
    }

    fn map_godot_property_hint(hint: &str) -> &'static str {
        match hint {
            "range" => "PROPERTY_HINT_RANGE",
            "enum" => "PROPERTY_HINT_ENUM",
            "file" => "PROPERTY_HINT_FILE",
            "dir" => "PROPERTY_HINT_DIR",
            "multiline" => "PROPERTY_HINT_MULTILINE_TEXT",
            _ => "PROPERTY_HINT_NONE",
        }
    }
}
