//! The import allowlist of contract v0 (FR-003, FR-004, research R4).
//!
//! A plugin may import only the `log` capability. Anything else is refused before linking, with
//! every refused import named. The check runs on names alone, so it also refuses imports the
//! linker would let through, such as an instance with nothing in it.

use wasmtime::Engine;
use wasmtime::component::Component;

use crate::error::{ErrorKind, RuntimeError};

/// Every import a plugin of contract v0 may have.
const ALLOWED: &[&str] = &["dashboard:widget/log@0.1.0"];

/// The start of every interface name of the plugin contract, whatever its version.
const CONTRACT: &str = "dashboard:widget/";

/// Refuses a component that imports anything outside [`ALLOWED`].
pub(crate) fn check(engine: &Engine, component: &Component) -> Result<(), RuntimeError> {
    let mut refused: Vec<String> = component
        .component_type()
        .imports(engine)
        .map(|(name, _)| name)
        .filter(|name| !ALLOWED.contains(name))
        .map(str::to_owned)
        .collect();
    if refused.is_empty() {
        return Ok(());
    }
    refused.sort();

    // Another version of the contract breaks a different rule than asking for a capability.
    if let Some(import) = refused.iter().find(|name| name.starts_with(CONTRACT)) {
        return Err(RuntimeError::new(
            ErrorKind::ContractMismatch,
            format!("built for another version of the plugin contract: imports {import}"),
        ));
    }
    Err(RuntimeError::capability_refused(refused))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_wat(wat: &str) -> Result<(), RuntimeError> {
        let engine = Engine::default();
        let component = Component::new(&engine, wat).expect("the test component compiles");
        check(&engine, &component)
    }

    #[test]
    fn allows_no_imports_and_the_log_capability() {
        assert_eq!(check_wat("(component)"), Ok(()));
        assert_eq!(
            check_wat(
                r#"(component (import "dashboard:widget/log@0.1.0" (instance
                     (export "write" (func (param "message" string))))))"#
            ),
            Ok(())
        );
    }

    #[test]
    fn refuses_an_empty_instance_the_linker_would_accept() {
        let error = check_wat(r#"(component (import "wasi:filesystem/types@0.2.0" (instance)))"#)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
        assert_eq!(error.refused_imports(), ["wasi:filesystem/types@0.2.0"]);
    }

    #[test]
    fn lists_every_refused_import_sorted() {
        let error = check_wat(
            r#"(component
                 (import "wasi:sockets/tcp@0.2.0" (instance))
                 (import "dashboard:widget/log@0.1.0" (instance))
                 (import "wasi:clocks/wall-clock@0.2.0" (instance)))"#,
        )
        .unwrap_err();
        assert_eq!(
            error.refused_imports(),
            ["wasi:clocks/wall-clock@0.2.0", "wasi:sockets/tcp@0.2.0"]
        );
    }

    #[test]
    fn another_contract_version_is_a_mismatch_not_a_capability() {
        let error = check_wat(
            r#"(component
                 (import "wasi:sockets/tcp@0.2.0" (instance))
                 (import "dashboard:widget/log@0.2.0" (instance)))"#,
        )
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ContractMismatch);
        assert!(error.detail().contains("dashboard:widget/log@0.2.0"));
    }
}
