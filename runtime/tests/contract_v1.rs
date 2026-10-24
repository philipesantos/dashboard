//! The rule of contract v1 (FR-013, research R7): every function of a capability interface
//! returns `result<T, E>`, where `E` is an enum or variant of that same interface whose first
//! case is `not-granted`. That is the answer of an optional capability that is off, and the
//! stand-in gives it for every function.

use std::path::Path;

use wit_parser::{Resolve, Type, TypeDefKind, TypeOwner};

/// The interfaces a plugin exports, which carry no capability.
const RENDERER: &str = "renderer";
const UPDATER: &str = "updater";
const NOT_GRANTED: &str = "not-granted";

/// Why a function breaks the rule, if it does.
fn check_function(resolve: &Resolve, owner: TypeOwner, result: Option<&Type>) -> Option<String> {
    let Some(Type::Id(result)) = result else {
        return Some("it does not return a result".to_owned());
    };
    let TypeDefKind::Result(result) = &resolve.types[*result].kind else {
        return Some("it does not return a result".to_owned());
    };
    let Some(Type::Id(error)) = &result.err else {
        return Some("its result has no error type".to_owned());
    };
    let error = &resolve.types[*error];
    if error.owner != owner {
        return Some("its error type belongs to another interface".to_owned());
    }
    let first_case = match &error.kind {
        TypeDefKind::Enum(error) => error.cases.first().map(|case| case.name.as_str()),
        TypeDefKind::Variant(error) => error.cases.first().map(|case| case.name.as_str()),
        _ => return Some("its error type is neither an enum nor a variant".to_owned()),
    };
    (first_case != Some(NOT_GRANTED))
        .then(|| format!("the first case of its error type is not `{NOT_GRANTED}`"))
}

#[test]
fn every_capability_of_contract_v1_can_answer_not_granted() {
    let wit = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("wit")
        .join("v1");
    let mut resolve = Resolve::new();
    let (package, _) = resolve
        .push_dir(&wit)
        .unwrap_or_else(|error| panic!("wit/v1 parses: {error:#}"));

    let mut capabilities = 0;
    let mut broken = Vec::new();
    for (name, &interface) in &resolve.packages[package].interfaces {
        if name == RENDERER || name == UPDATER {
            continue;
        }
        capabilities += 1;
        for function in resolve.interfaces[interface].functions.values() {
            if let Some(problem) = check_function(
                &resolve,
                TypeOwner::Interface(interface),
                function.result.as_ref(),
            ) {
                broken.push(format!("{name}.{}: {problem}", function.name));
            }
        }
    }
    assert!(capabilities > 0, "wit/v1 has no capability interface");
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn the_check_refuses_a_capability_that_cannot_answer_not_granted() {
    let broken = [
        // v0's log: no result at all
        "interface log { write: func(message: string); }",
        // A result without an error
        "interface clock { now: func() -> result<u64>; }",
        // An error whose first case is another one
        "interface net { enum net-error { timeout, not-granted } get: func() -> result<string, net-error>; }",
        // An error that is a plain string
        "interface store { get: func() -> result<string, string>; }",
    ];
    for source in broken {
        let mut resolve = Resolve::new();
        let package = resolve
            .push_str("test.wit", &format!("package test:broken@1.0.0;\n{source}"))
            .expect("the test WIT parses");
        let found = resolve.packages[package]
            .interfaces
            .values()
            .flat_map(|&interface| {
                resolve.interfaces[interface]
                    .functions
                    .values()
                    .map(move |function| (interface, function))
            })
            .any(|(interface, function)| {
                check_function(
                    &resolve,
                    TypeOwner::Interface(interface),
                    function.result.as_ref(),
                )
                .is_some()
            });
        assert!(found, "the check accepted: {source}");
    }
}

/// research R11: a minor version only adds. 1.1.0 (`wit/v1`) holds every interface, function
/// and type of the published 1.0.0 (`wit/history/1.0.0`) unchanged, compared as resolved
/// definitions; doc comments, which 1.1.0 rewrites, are not part of them.
#[test]
fn contract_1_1_0_contains_everything_of_1_0_0_unchanged() {
    let wit = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("wit");
    let old = definitions(&wit.join("history").join("1.0.0"));
    let new = definitions(&wit.join("v1"));
    assert!(!old.is_empty());
    let mut missing = Vec::new();
    for (name, definition) in &old {
        match new.get(name) {
            None => missing.push(format!("{name} is gone")),
            Some(changed) if changed != definition => {
                missing.push(format!(
                    "{name} changed:\n  1.0.0: {definition}\n  1.1.0: {changed}"
                ));
            }
            Some(_) => {}
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
    // And 1.1.0 adds connectors and updater
    for added in ["connectors.call", "updater.update", "connectors.call-error"] {
        assert!(new.contains_key(added), "{added}");
    }
}

/// Every function and named type of the package in `dir`, as `interface.name` → its resolved
/// definition, docs left out.
fn definitions(dir: &Path) -> std::collections::BTreeMap<String, String> {
    let mut resolve = Resolve::new();
    let (package, _) = resolve
        .push_dir(dir)
        .unwrap_or_else(|error| panic!("{} parses: {error:#}", dir.display()));
    let mut definitions = std::collections::BTreeMap::new();
    for (interface_name, &interface) in &resolve.packages[package].interfaces {
        let interface = &resolve.interfaces[interface];
        for (name, function) in &interface.functions {
            let params: Vec<String> = function
                .params
                .iter()
                .map(|param| format!("{}: {}", param.name, render(&resolve, &param.ty)))
                .collect();
            let result = function
                .result
                .as_ref()
                .map(|result| render(&resolve, result))
                .unwrap_or_default();
            definitions.insert(
                format!("{interface_name}.{name}"),
                format!("func({}) -> {result}", params.join(", ")),
            );
        }
        for (name, &id) in &interface.types {
            definitions.insert(
                format!("{interface_name}.{name}"),
                define(&resolve, &resolve.types[id].kind),
            );
        }
    }
    for (world_name, &world) in &resolve.packages[package].worlds {
        let world = &resolve.worlds[world];
        let mut items: Vec<String> = world
            .imports
            .keys()
            .map(|key| format!("import {}", resolve.name_world_key(key)))
            .chain(
                world
                    .exports
                    .keys()
                    .map(|key| format!("export {}", resolve.name_world_key(key))),
            )
            .map(|item| item.split('@').next().unwrap().to_owned())
            .collect();
        items.sort();
        definitions.insert(format!("world {world_name}"), items.join("; "));
    }
    definitions
}

/// A type where it is used: a named type by its name, an anonymous one by its structure.
fn render(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Id(id) => {
            let def = &resolve.types[*id];
            match &def.name {
                Some(name) => name.clone(),
                None => define(resolve, &def.kind),
            }
        }
        other => format!("{other:?}").to_lowercase(),
    }
}

/// A type's whole definition.
fn define(resolve: &Resolve, kind: &TypeDefKind) -> String {
    let optional = |ty: &Option<Type>| {
        ty.as_ref()
            .map(|ty| render(resolve, ty))
            .unwrap_or("_".to_owned())
    };
    match kind {
        TypeDefKind::Record(record) => {
            let fields: Vec<String> = record
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, render(resolve, &field.ty)))
                .collect();
            format!("record {{ {} }}", fields.join(", "))
        }
        TypeDefKind::Enum(enumeration) => {
            let cases: Vec<&str> = enumeration
                .cases
                .iter()
                .map(|case| case.name.as_str())
                .collect();
            format!("enum {{ {} }}", cases.join(", "))
        }
        TypeDefKind::Variant(variant) => {
            let cases: Vec<String> = variant
                .cases
                .iter()
                .map(|case| format!("{}({})", case.name, optional(&case.ty)))
                .collect();
            format!("variant {{ {} }}", cases.join(", "))
        }
        TypeDefKind::Flags(flags) => {
            let names: Vec<&str> = flags.flags.iter().map(|flag| flag.name.as_str()).collect();
            format!("flags {{ {} }}", names.join(", "))
        }
        TypeDefKind::Tuple(tuple) => {
            let types: Vec<String> = tuple.types.iter().map(|ty| render(resolve, ty)).collect();
            format!("tuple<{}>", types.join(", "))
        }
        TypeDefKind::Option(ty) => format!("option<{}>", render(resolve, ty)),
        TypeDefKind::Result(result) => format!(
            "result<{}, {}>",
            optional(&result.ok),
            optional(&result.err)
        ),
        TypeDefKind::List(ty) => format!("list<{}>", render(resolve, ty)),
        TypeDefKind::Type(ty) => format!("type = {}", render(resolve, ty)),
        other => format!("{other:?}"),
    }
}
