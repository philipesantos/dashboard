//! The rule of contract v1 (FR-013, research R7): every function of a capability interface
//! returns `result<T, E>`, where `E` is an enum or variant of that same interface whose first
//! case is `not-granted`. That is the answer of an optional capability that is off, and the
//! stand-in gives it for every function.

use std::path::Path;

use wit_parser::{Resolve, Type, TypeDefKind, TypeOwner};

/// The interface every plugin implements, which carries no capability.
const RENDERER: &str = "renderer";
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
        if name == RENDERER {
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
