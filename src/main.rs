use clang::*;
use walkdir::WalkDir;
use heck::SnakeCase;

fn main() {
    let prefix = std::env::args().nth(1);

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

    let functions = sonar::find_functions(entities);
    for function in functions {
        let func_name = (if let Some(prefix) = &prefix {
            if !function.name.starts_with::<&str>(prefix.as_ref()) { continue; }
            if function.name.ends_with("Initialize") { continue; }
            if function.name.ends_with("Exit") { continue; }
            (&function.name[prefix.len()..]).to_string()
        } else {
            function.name
        }).to_snake_case();

        print!("pub fn {}(&mut self", &func_name);

        for argument in function.entity.get_arguments().unwrap_or_else(Vec::new) {
            let typ = argument.get_type().unwrap();

            let (is_pointer, typ) = if let Some(ptr) = typ.get_pointee_type() {
                (true, ptr)
            } else {
                (false, typ)
            };

            print!(", {}: ", argument.get_name().unwrap());

            let typ_name = match typ.get_display_name().as_ref() {
                "Result" => "LibnxResult".to_string(),
                "int" => "i32".to_string(),
                "void" => "()".to_string(),
                "const void" => "const ()".to_string(),
                other => other.to_string(),
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

        println!("{{ unimplemented!() }}");
    }
}
