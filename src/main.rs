use clang::*;
use walkdir::WalkDir;
use heck::SnakeCase;
use std::collections::HashSet;

fn invoke_handle<'tu>(functions: sonar::Functions<'tu>, prefix: &str) {
    let mut init = None;
    let mut exit = None;

    for func in functions {
        if !func.name.starts_with(prefix) { continue; }

        if func.name.ends_with("Initialize") || func.name.ends_with("Init") {
            init = Some(func);
        } else if func.name.ends_with("Exit") {
            exit = Some(func);
        }

        if init.is_some() && exit.is_some() { break; }
    }

    let init = init.unwrap();
    let exit = exit.unwrap();

    print!("handle!(");

    if init.entity.get_result_type().unwrap().get_display_name() == "Result" {
        print!("0");
    } else {
        print!("_");
    }

    print!(" in sys::{}(", init.name);

    let mut first = true;

    for argument in init.entity.get_arguments().unwrap() {
        if !first {
            print!(", ");
        }

        first = true;

        let typ = argument.get_type().unwrap();
        if typ.get_pointee_type().is_some() {
            if typ.get_display_name().starts_with("const ") {
                print!("ptr::null()");
            } else {
                print!("ptr::null_mut()");
            }
        } else {
            print!("Default::default()");
        }
    }

    print!("), sys::{}(", exit.name);

    for argument in exit.entity.get_arguments().unwrap() {
        if !first {
            print!(", ");
        }

        first = true;

        let typ = argument.get_type().unwrap();
        if typ.get_pointee_type().is_some() {
            if typ.get_display_name().starts_with("const ") {
                print!("ptr::null()");
            } else {
                print!("ptr::null_mut()");
            }
        } else {
            print!("Default::default()");
        }
    }

    println!("), {{");
}

fn main() {
    let prefix = std::env::args().nth(1);

    if prefix.is_some() {
        println!("use crate::sys;");
        println!("use crate::macros::handle;");
        println!("use std::ptr;\n");
    }

    let clang = Clang::new().unwrap();
    let index = Index::new(&clang, false, false);

    let mut units = Vec::new();
    for entry in WalkDir::new("libnx/nx/include/") {
        let entry = entry.unwrap();
        if entry.file_type().is_dir() { continue; }
        let parser = index.parser(entry.into_path());
        let unit = parser.parse().unwrap();
        units.push(unit);
    }

    let entities: Vec<_> = units.iter().flat_map(|e| e.get_entity().get_children()).collect();

    if let Some(prefix) = &prefix {
        invoke_handle(sonar::find_functions(entities.clone()), prefix);
    }

    let mut exported_types = HashSet::new();
    let mut first = true;

    let functions = sonar::find_functions(entities);
    for function in functions {
        let func_name = (if let Some(prefix) = &prefix {
            if !function.name.starts_with::<&str>(prefix.as_ref()) { continue; }
            if function.name.ends_with("Initialize") { continue; }
            if function.name.ends_with("Exit") { continue; }
            (&function.name[prefix.len()..]).to_string()
        } else {
            function.name.clone()
        }).to_snake_case();

        if !first {
            print!("\n");
        }

        first = false;

        let mut args = Vec::new();

        if prefix.is_some() { print!("    "); }
        print!("pub fn {}(&mut self", &func_name);

        for argument in function.entity.get_arguments().unwrap_or_else(Vec::new) {
            let typ = argument.get_type().unwrap();

            let (is_pointer, typ) = if let Some(ptr) = typ.get_pointee_type() {
                (true, ptr)
            } else {
                (false, typ)
            };

            args.push(argument.get_name().unwrap());

            print!(", {}: ", argument.get_name().unwrap());

            let typ_name = match typ.get_display_name().as_ref() {
                "Result" => "LibnxResult".to_string(),
                "int" => "i32".to_string(),
                "void" => "()".to_string(),
                "const void" => "const ()".to_string(),
                "u8" => "u8".to_string(),
                "u16" => "u16".to_string(),
                "u32" => "u32".to_string(),
                "u64" => "u64".to_string(),
                "i8" => "i8".to_string(),
                "i16" => "i16".to_string(),
                "i32" => "i32".to_string(),
                "i64" => "i64".to_string(),
                "const char" => "const char".to_string(),
                "char" => "char".to_string(),
                other => {
                    exported_types.insert(other.to_string());
                    other.to_string()
                },
            };

            if is_pointer && typ_name.starts_with("const") {
                print!("*{}", typ_name);
            } else {
                if is_pointer {
                    print!("*mut {}", typ_name);
                } else {
                    print!("{}", typ_name);
                }
            }
        }

        print!(") ");

        if let Some(ret) = function.entity.get_result_type() {
            let (is_pointer, ret) = if let Some(ptr) = ret.get_pointee_type() {
                (true, ptr)
            } else {
                (false, ret)
            };

            let name = match ret.get_display_name().as_ref() {
                "Result" => "LibnxResult".to_string(),
                "int" => "i32".to_string(),
                other => other.to_string(),
            };

            match (name.as_ref(), is_pointer) {
                ("void", false) => {},
                (name, false) => { print!("-> {} ", name) },
                ("void", true) => { print!("-> *mut () ") },
                (name, true) => { print!("-> *mut {} ", name) },
            }
        }

        println!("{{");

        if prefix.is_some() { print!("    "); }
        print!("    sys::{}(", function.name);

        let mut first = true;

        for arg in args {
            if !first {
                print!(", ");
            }

            first = false;

            print!("{}", arg);
        }

        println!(").into()");

        if prefix.is_some() { print!("    "); }
        println!("}}");
    }

    if prefix.is_some() {
        println!("}});\n");

        for export in exported_types {
            println!("pub use sys::{};", export);
        }
    }
}
