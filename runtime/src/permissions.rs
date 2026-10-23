//! The permission engine (`specs/002-plugin-permissions/research.md`, R5).
//!
//! The registry is every permission this host knows, each tied to the interface that carries it
//! in each contract version. A plugin can import an interface only through a permission its
//! manifest declares and the user granted. The load decision runs on names alone, so it also
//! refuses imports the linker would let through, such as an instance with nothing in it.

use wasmtime::Engine;
use wasmtime::component::Component;

use crate::error::{ErrorKind, RuntimeError};
use crate::grants::Entry;
use crate::manifest::Manifest;

/// A version of the plugin contract the host runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contract {
    /// `dashboard:widget@0.1.0` (`wit/v0`): only `log`, which cannot answer "not granted".
    V0,
    /// `dashboard:widget@1.0.0` (`wit/v1`): every capability can answer "not granted".
    V1,
}

impl Contract {
    /// Every version, oldest first.
    pub const ALL: [Contract; 2] = [Contract::V0, Contract::V1];

    /// The version's name, as the CLI prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            Contract::V0 => "v0",
            Contract::V1 => "v1",
        }
    }

    /// The interface every plugin of this version exports.
    pub fn renderer(self) -> &'static str {
        match self {
            Contract::V0 => "dashboard:widget/renderer@0.1.0",
            Contract::V1 => "dashboard:widget/renderer@1.0.0",
        }
    }

    /// The version whose `renderer` is `export`, matched exactly.
    pub fn from_renderer(export: &str) -> Option<Contract> {
        Contract::ALL
            .into_iter()
            .find(|contract| contract.renderer() == export)
    }
}

/// A permission the host knows.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Permission {
    /// The manifest's name for it.
    pub(crate) name: &'static str,
    /// The interface that carries it in contract v0, if v0 has it.
    v0: Option<&'static str>,
    /// The interface that carries it in contract v1.
    v1: &'static str,
}

impl Permission {
    /// The interface that carries this permission in `contract`, if that version has it.
    pub(crate) fn interface(&self, contract: Contract) -> Option<&'static str> {
        match contract {
            Contract::V0 => self.v0,
            Contract::V1 => Some(self.v1),
        }
    }
}

/// Every permission this host knows. Features 003–005 add one row each, for contract v1 only.
const REGISTRY: &[Permission] = &[Permission {
    name: "log",
    v0: Some("dashboard:widget/log@0.1.0"),
    v1: "dashboard:widget/log@1.0.0",
}];

/// The names a component imports and exports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Interfaces {
    pub(crate) imports: Vec<String>,
    pub(crate) exports: Vec<String>,
}

impl Interfaces {
    pub(crate) fn of(engine: &Engine, component: &Component) -> Interfaces {
        let component_type = component.component_type();
        Interfaces {
            imports: component_type
                .imports(engine)
                .map(|(name, _)| name.to_owned())
                .collect(),
            exports: component_type
                .exports(engine)
                .map(|(name, _)| name.to_owned())
                .collect(),
        }
    }
}

/// What a plugin is linked with: for each permission it imports, the real interface or the
/// stand-in, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkPlan {
    pub(crate) contract: Contract,
    /// Granted permissions the component imports, linked to their real interface.
    pub(crate) real: Vec<&'static str>,
    /// Optional permissions that are off and that the component imports, linked to the
    /// stand-in that answers `not-granted` (FR-019).
    pub(crate) stand_in: Vec<&'static str>,
}

/// The outcome of load steps 7–9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) plan: LinkPlan,
    /// Granted permissions this manifest no longer declares, which the record must drop
    /// (FR-012).
    pub(crate) prune: Vec<String>,
}

/// Load steps 5 and 6 (`data-model.md`, Load decision): the contract, then the imports. Needs
/// no grants, so `approve` runs it too.
pub(crate) fn check_component(
    _interfaces: &Interfaces,
    _manifest: &Manifest,
) -> Result<Contract, RuntimeError> {
    Err(RuntimeError::new(
        ErrorKind::ContractMismatch,
        "not implemented yet",
    ))
}

/// Load steps 7–9: the plugin's approval from `origin`, then what to prune and what to link.
pub(crate) fn decide(
    _contract: Contract,
    _interfaces: &Interfaces,
    _manifest: &Manifest,
    _origin: &str,
    _entry: Option<&Entry>,
) -> Result<Decision, RuntimeError> {
    Err(RuntimeError::new(
        ErrorKind::NotGranted,
        "not implemented yet",
    ))
}

/// The permission named `name`, if the host knows it.
pub(crate) fn by_name(name: &str) -> Option<&'static Permission> {
    REGISTRY.iter().find(|permission| permission.name == name)
}

/// The permission whose interface in `contract` is `import`, matched exactly.
pub(crate) fn by_import(contract: Contract, import: &str) -> Option<&'static Permission> {
    REGISTRY
        .iter()
        .find(|permission| permission.interface(contract) == Some(import))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::grants::GrantStore;

    const ORIGIN: &str = "/plugins/probe.wasm";
    const OTHER_ORIGIN: &str = "/downloads/probe.wasm";
    const DIGEST: &str = "sha256:00";

    /// The names of a WAT component with these `imports`, exporting `renderer`.
    fn interfaces(imports: &[&str], renderer: Option<&str>) -> Interfaces {
        let mut wat = String::from("(component");
        for import in imports {
            wat.push_str(&format!(" (import \"{import}\" (instance))"));
        }
        if let Some(renderer) = renderer {
            wat.push_str(&format!(
                " (instance $r) (export \"{renderer}\" (instance $r))"
            ));
        }
        wat.push(')');
        let engine = Engine::default();
        let component = Component::new(&engine, wat).expect("the test component compiles");
        Interfaces::of(&engine, &component)
    }

    fn v0(imports: &[&str]) -> Interfaces {
        interfaces(imports, Some(Contract::V0.renderer()))
    }

    fn v1(imports: &[&str]) -> Interfaces {
        interfaces(imports, Some(Contract::V1.renderer()))
    }

    const LOG_V0: &str = "dashboard:widget/log@0.1.0";
    const LOG_V1: &str = "dashboard:widget/log@1.0.0";

    /// A manifest for `local/probe` with `log` required (`Some(true)`), optional or undeclared.
    fn manifest(log: Option<bool>) -> Manifest {
        let permissions: Vec<serde_json::Value> = log
            .map(|required| json!({ "name": "log", "required": required }))
            .into_iter()
            .collect();
        let manifest = json!({
            "manifest_version": 1,
            "id": "local/probe",
            "version": "1.0.0",
            "author": "A",
            "permissions": permissions,
            "login_providers": []
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    fn refusal<T: std::fmt::Debug>(result: Result<T, RuntimeError>) -> (ErrorKind, Vec<String>) {
        let error = result.expect_err("refused");
        (error.kind(), error.names().to_vec())
    }

    /// An in-memory store where `local/probe` was approved from [`ORIGIN`] under `approved`,
    /// with `log` turned on when `log_on`.
    fn approved(approved: &Manifest, log_on: bool) -> GrantStore {
        let store = GrantStore::in_memory();
        store.approve(approved, ORIGIN, DIGEST).expect("approved");
        if log_on {
            store
                .set_optional(approved, ORIGIN, "log", true)
                .expect("turned on");
        }
        store
    }

    fn entry(store: &GrantStore) -> Option<Entry> {
        store
            .entry(&crate::manifest::PluginId::parse("local/probe").expect("valid"))
            .expect("readable")
    }

    /// Steps 5–9 for a component and a manifest, against `store`'s approval from `origin`.
    fn load(
        interfaces: &Interfaces,
        manifest: &Manifest,
        store: &GrantStore,
        origin: &str,
    ) -> Result<Decision, RuntimeError> {
        let contract = check_component(interfaces, manifest)?;
        decide(
            contract,
            interfaces,
            manifest,
            origin,
            entry(store).as_ref(),
        )
    }

    #[test]
    fn the_renderer_export_sets_the_contract() {
        assert_eq!(check_component(&v0(&[]), &manifest(None)), Ok(Contract::V0));
        assert_eq!(check_component(&v1(&[]), &manifest(None)), Ok(Contract::V1));
    }

    #[test]
    fn no_renderer_of_a_known_version_is_a_contract_mismatch() {
        for renderer in [
            None,
            Some("dashboard:widget/renderer@1.1.0"),
            Some("dashboard:widget/renderer@2.0.0"),
            Some("dashboard:other/renderer@1.0.0"),
        ] {
            let error =
                check_component(&interfaces(&[], renderer), &manifest(None)).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::ContractMismatch, "{renderer:?}");
        }
    }

    #[test]
    fn an_import_from_another_contract_version_is_a_contract_mismatch() {
        for (component, import) in [
            (
                v1(&["dashboard:widget/log@1.1.0"]),
                "dashboard:widget/log@1.1.0",
            ),
            (v1(&[LOG_V0]), LOG_V0),
            (v0(&[LOG_V1]), LOG_V1),
            (
                v0(&["dashboard:widget/log@0.2.0"]),
                "dashboard:widget/log@0.2.0",
            ),
            // Even beside a capability the host refuses: another version breaks another rule
            (
                v1(&["wasi:sockets/tcp@0.2.0", "dashboard:widget/log@1.1.0"]),
                "dashboard:widget/log@1.1.0",
            ),
        ] {
            let error = check_component(&component, &manifest(Some(true))).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::ContractMismatch, "{import}");
            assert!(error.detail().contains(import), "{}", error.detail());
        }
    }

    #[test]
    fn a_v0_plugin_with_an_optional_permission_needs_v1() {
        assert_eq!(
            refusal(check_component(&v0(&[LOG_V0]), &manifest(Some(false)))),
            (ErrorKind::OptionalNeedsV1, vec!["log".to_owned()])
        );
        // Whether it imports the permission or not
        assert_eq!(
            refusal(check_component(&v0(&[]), &manifest(Some(false)))).0,
            ErrorKind::OptionalNeedsV1
        );
        assert_eq!(
            check_component(&v1(&[LOG_V1]), &manifest(Some(false))),
            Ok(Contract::V1)
        );
    }

    #[test]
    fn an_import_the_host_does_not_offer_is_refused_and_every_one_is_listed() {
        // An empty instance, which the linker would accept
        assert_eq!(
            refusal(check_component(
                &v1(&["wasi:filesystem/types@0.2.0"]),
                &manifest(None)
            )),
            (
                ErrorKind::CapabilityRefused,
                vec!["wasi:filesystem/types@0.2.0".to_owned()]
            )
        );
        assert_eq!(
            refusal(check_component(
                &v0(&[
                    "wasi:sockets/tcp@0.2.0",
                    LOG_V0,
                    "wasi:clocks/wall-clock@0.2.0"
                ]),
                &manifest(Some(true))
            )),
            (
                ErrorKind::CapabilityRefused,
                vec![
                    "wasi:clocks/wall-clock@0.2.0".to_owned(),
                    "wasi:sockets/tcp@0.2.0".to_owned()
                ]
            )
        );
        // An interface of the contract that carries no permission
        assert_eq!(
            refusal(check_component(
                &v1(&[
                    "dashboard:widget/renderer@1.0.0",
                    "dashboard:widget/telepathy@1.0.0"
                ]),
                &manifest(None)
            )),
            (
                ErrorKind::CapabilityRefused,
                vec![
                    "dashboard:widget/renderer@1.0.0".to_owned(),
                    "dashboard:widget/telepathy@1.0.0".to_owned()
                ]
            )
        );
    }

    #[test]
    fn an_import_whose_permission_is_not_declared_is_refused() {
        assert_eq!(
            refusal(check_component(&v1(&[LOG_V1]), &manifest(None))),
            (ErrorKind::NotDeclared, vec!["log".to_owned()])
        );
        assert_eq!(
            refusal(check_component(&v0(&[LOG_V0]), &manifest(None))),
            (ErrorKind::NotDeclared, vec!["log".to_owned()])
        );
    }

    #[test]
    fn a_plugin_without_an_approval_is_not_granted() {
        let store = GrantStore::in_memory();
        assert_eq!(
            refusal(load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, ORIGIN)),
            (ErrorKind::NotGranted, vec!["log".to_owned()])
        );
        // Even one that requests nothing: every plugin needs an approval
        let error = load(&v1(&[]), &manifest(None), &store, ORIGIN).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::NotGranted);
        assert!(error.detail().contains("local/probe"), "{}", error.detail());
    }

    #[test]
    fn an_approval_from_another_origin_grants_nothing() {
        let store = approved(&manifest(Some(true)), false);
        let error =
            load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, OTHER_ORIGIN).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::NotGranted);
        assert_eq!(error.names(), ["log"]);
        assert!(error.detail().contains(ORIGIN), "{}", error.detail());

        let store = approved(&manifest(Some(false)), true);
        let error = load(&v1(&[LOG_V1]), &manifest(Some(false)), &store, OTHER_ORIGIN)
            .expect_err("an optional permission on is not granted elsewhere either");
        assert_eq!(error.kind(), ErrorKind::NotGranted);
    }

    #[test]
    fn a_new_required_permission_needs_a_new_approval() {
        let store = approved(&manifest(None), false);
        assert_eq!(
            refusal(load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, ORIGIN)),
            (ErrorKind::NeedsNewApproval, vec!["log".to_owned()])
        );
        // An optional permission that was off and is now required
        let store = approved(&manifest(Some(false)), false);
        assert_eq!(
            refusal(load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, ORIGIN)).0,
            ErrorKind::NeedsNewApproval
        );
    }

    #[test]
    fn a_required_permission_must_be_granted_even_if_not_imported() {
        let store = approved(&manifest(None), false);
        assert_eq!(
            refusal(load(&v1(&[]), &manifest(Some(true)), &store, ORIGIN)),
            (ErrorKind::NeedsNewApproval, vec!["log".to_owned()])
        );
    }

    #[test]
    fn the_link_plan_has_the_real_interface_or_the_stand_in() {
        let store = approved(&manifest(Some(true)), false);
        let decision = load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, ORIGIN).expect("runs");
        assert_eq!(
            decision.plan,
            LinkPlan {
                contract: Contract::V1,
                real: vec!["log"],
                stand_in: vec![]
            }
        );

        let store = approved(&manifest(Some(true)), false);
        let decision = load(&v0(&[LOG_V0]), &manifest(Some(true)), &store, ORIGIN).expect("runs");
        assert_eq!(decision.plan.contract, Contract::V0);
        assert_eq!(decision.plan.real, ["log"]);

        let store = approved(&manifest(Some(false)), false);
        let decision = load(&v1(&[LOG_V1]), &manifest(Some(false)), &store, ORIGIN).expect("runs");
        assert_eq!(decision.plan.real, Vec::<&str>::new());
        assert_eq!(decision.plan.stand_in, ["log"]);
    }

    #[test]
    fn pruning_names_what_the_record_must_drop() {
        let store = approved(&manifest(Some(true)), false);
        let decision = load(&v1(&[]), &manifest(None), &store, ORIGIN).expect("runs");
        assert_eq!(decision.prune, ["log"]);
        assert_eq!(decision.plan.real, Vec::<&str>::new());

        let decision = load(&v1(&[LOG_V1]), &manifest(Some(true)), &store, ORIGIN).expect("runs");
        assert!(decision.prune.is_empty());
    }

    /// `data-model.md`, Combinations (SC-002): one permission, declared or not, granted or not,
    /// imported or not, for a plugin approved from its own origin.
    #[test]
    fn every_combination_of_declared_granted_and_imported() {
        // (declared, granted, imported) → Ok((real, stand-in, prune)) or the refusal's kind
        type Outcome = Result<(Vec<&'static str>, Vec<&'static str>, Vec<&'static str>), ErrorKind>;
        let rows: [(Option<bool>, bool, bool, Outcome); 12] = [
            (None, false, false, Ok((vec![], vec![], vec![]))),
            (None, true, false, Ok((vec![], vec![], vec!["log"]))),
            (None, false, true, Err(ErrorKind::NotDeclared)),
            (None, true, true, Err(ErrorKind::NotDeclared)),
            (Some(true), true, true, Ok((vec!["log"], vec![], vec![]))),
            (Some(true), true, false, Ok((vec![], vec![], vec![]))),
            (Some(true), false, true, Err(ErrorKind::NeedsNewApproval)),
            (Some(true), false, false, Err(ErrorKind::NeedsNewApproval)),
            (Some(false), true, true, Ok((vec!["log"], vec![], vec![]))),
            (Some(false), true, false, Ok((vec![], vec![], vec![]))),
            (Some(false), false, true, Ok((vec![], vec!["log"], vec![]))),
            (Some(false), false, false, Ok((vec![], vec![], vec![]))),
        ];
        for (declared, granted, imported, expected) in rows {
            // The approval: log granted (required, or optional and on), or not at all
            let store = if granted {
                approved(&manifest(Some(false)), true)
            } else {
                approved(&manifest(None), false)
            };
            let imports: &[&str] = if imported { &[LOG_V1] } else { &[] };
            let outcome = load(&v1(imports), &manifest(declared), &store, ORIGIN)
                .map(|decision| {
                    (
                        decision.plan.real,
                        decision.plan.stand_in,
                        decision.prune.iter().map(|_| "log").collect(),
                    )
                })
                .map_err(|error| error.kind());
            assert_eq!(
                outcome, expected,
                "declared {declared:?}, granted {granted}, imported {imported}"
            );
        }

        // And without an approval, whatever the plugin declares
        for declared in [None, Some(true), Some(false)] {
            let outcome = load(
                &v1(&[]),
                &manifest(declared),
                &GrantStore::in_memory(),
                ORIGIN,
            );
            assert_eq!(refusal(outcome).0, ErrorKind::NotGranted, "{declared:?}");
        }
    }

    #[test]
    fn each_contract_has_its_renderer() {
        assert_eq!(Contract::V0.renderer(), "dashboard:widget/renderer@0.1.0");
        assert_eq!(Contract::V1.renderer(), "dashboard:widget/renderer@1.0.0");
        assert_eq!(
            Contract::from_renderer("dashboard:widget/renderer@0.1.0"),
            Some(Contract::V0)
        );
        assert_eq!(
            Contract::from_renderer("dashboard:widget/renderer@1.0.0"),
            Some(Contract::V1)
        );
        assert_eq!(Contract::V0.as_str(), "v0");
        assert_eq!(Contract::V1.as_str(), "v1");
    }

    #[test]
    fn another_renderer_version_is_no_contract() {
        for export in [
            "dashboard:widget/renderer@1.1.0",
            "dashboard:widget/renderer@0.2.0",
            "dashboard:widget/renderer",
            "dashboard:widget/log@1.0.0",
        ] {
            assert_eq!(Contract::from_renderer(export), None, "{export}");
        }
    }

    #[test]
    fn finds_log_by_name() {
        let log = by_name("log").expect("log is registered");
        assert_eq!(log.name, "log");
        assert_eq!(
            log.interface(Contract::V0),
            Some("dashboard:widget/log@0.1.0")
        );
        assert_eq!(
            log.interface(Contract::V1),
            Some("dashboard:widget/log@1.0.0")
        );
    }

    #[test]
    fn finds_log_by_import_in_each_contract() {
        assert_eq!(
            by_import(Contract::V0, "dashboard:widget/log@0.1.0").map(|p| p.name),
            Some("log")
        );
        assert_eq!(
            by_import(Contract::V1, "dashboard:widget/log@1.0.0").map(|p| p.name),
            Some("log")
        );
    }

    #[test]
    fn an_import_from_the_other_contract_matches_nothing() {
        assert_eq!(by_import(Contract::V0, "dashboard:widget/log@1.0.0"), None);
        assert_eq!(by_import(Contract::V1, "dashboard:widget/log@0.1.0"), None);
        assert_eq!(by_import(Contract::V1, "dashboard:widget/log@1.1.0"), None);
    }

    #[test]
    fn an_unknown_name_or_import_matches_nothing() {
        assert_eq!(by_name("telepathy"), None);
        assert_eq!(by_name("Log"), None);
        assert_eq!(by_name(""), None);
        assert_eq!(by_import(Contract::V1, "wasi:filesystem/types@0.2.0"), None);
    }
}
