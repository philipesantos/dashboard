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
    /// `dashboard:widget@1.m.p` (`wit/v1`), any minor up to the host's own, 1.1.0: every
    /// capability can answer "not granted".
    V1,
}

/// The newest minor version of contract v1 this host runs; plugins built for any v1 minor up to
/// it run, and newer ones are refused (FR-016).
pub const V1_VERSION: semver::Version = semver::Version::new(1, 1, 0);
/// The only version of contract v0.
const V0_VERSION: semver::Version = semver::Version::new(0, 1, 0);
/// The minor version that brought connectors and `updater`.
const CONNECTORS_SINCE: semver::Version = semver::Version::new(1, 1, 0);

/// The interface names of the contract, without their version.
const RENDERER: &str = "dashboard:widget/renderer";
const UPDATER: &str = "dashboard:widget/updater";
/// The interface that carries every connector a manifest declares (research R12).
const CONNECTORS: &str = "dashboard:widget/connectors";

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

    /// The `renderer` export of the newest version of this contract the host runs.
    pub fn renderer(self) -> &'static str {
        match self {
            Contract::V0 => "dashboard:widget/renderer@0.1.0",
            Contract::V1 => "dashboard:widget/renderer@1.1.0",
        }
    }

    /// The version whose `renderer` is `export`: v0 at exactly 0.1.0, v1 at any minor up to
    /// the host's own.
    pub fn from_renderer(export: &str) -> Option<Contract> {
        let (base, version) = split(export)?;
        (base == RENDERER).then_some(())?;
        contract_of(&version).filter(|_| version.major != 1 || version <= V1_VERSION)
    }
}

/// The contract version a plugin is built for: its major, and the exact version every one of
/// its contract names carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractVersion {
    pub contract: Contract,
    pub version: semver::Version,
}

impl ContractVersion {
    /// `v0`, or `v1 (1.0.0)`: how the CLI prints it.
    pub fn display(&self) -> String {
        match self.contract {
            Contract::V0 => self.contract.as_str().to_owned(),
            Contract::V1 => format!("{} ({})", self.contract.as_str(), self.version),
        }
    }

    /// Whether this version has connectors and `updater`.
    pub fn has_connectors(&self) -> bool {
        self.contract == Contract::V1 && self.version >= CONNECTORS_SINCE
    }
}

/// The contract a version belongs to, before checking it is one the host runs.
fn contract_of(version: &semver::Version) -> Option<Contract> {
    if *version == V0_VERSION {
        Some(Contract::V0)
    } else if version.major == 1 && version.pre.is_empty() && version.build.is_empty() {
        Some(Contract::V1)
    } else {
        None
    }
}

/// `dashboard:widget/log@1.0.0` as `("dashboard:widget/log", 1.0.0)`.
fn split(name: &str) -> Option<(&str, semver::Version)> {
    let (base, version) = name.rsplit_once('@')?;
    Some((base, semver::Version::parse(version).ok()?))
}

/// A permission the host knows.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Permission {
    /// The manifest's name for it.
    pub(crate) name: &'static str,
    /// The interface that carries it in contract v0, if v0 has it.
    v0: Option<&'static str>,
    /// The interface that carries it in contract v1, without its version.
    v1: &'static str,
    /// The v1 minor version that brought it.
    since: semver::Version,
}

/// The start of every interface name of the plugin contract, whatever its version.
const CONTRACT_PREFIX: &str = "dashboard:widget/";

/// Every permission this host knows by name. Each connector is a permission too, but it is
/// declared under `connectors` in the manifest and carried by the one `connectors` interface
/// (research R12), so it has no row here.
const REGISTRY: &[Permission] = &[Permission {
    name: "log",
    v0: Some("dashboard:widget/log@0.1.0"),
    v1: "dashboard:widget/log",
    since: semver::Version::new(1, 0, 0),
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

    /// The `renderer` and `updater` exports, by the exact names the component has.
    pub(crate) fn export(&self, base: &str) -> Option<&str> {
        self.exports
            .iter()
            .find(|export| split(export).is_some_and(|(name, _)| name == base))
            .map(String::as_str)
    }

    pub(crate) fn renderer(&self) -> Option<&str> {
        self.export(RENDERER)
    }

    pub(crate) fn updater(&self) -> Option<&str> {
        self.export(UPDATER)
    }
}

/// Whether `import` is the `connectors` interface of some v1 version.
fn is_connectors(import: &str) -> bool {
    split(import).is_some_and(|(base, _)| base == CONNECTORS)
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
    /// Whether the component imports `connectors`, linked over the table of granted connectors
    /// or to the stand-in (research R12).
    pub(crate) connectors: bool,
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
    interfaces: &Interfaces,
    manifest: &Manifest,
) -> Result<ContractVersion, RuntimeError> {
    // Step 5: the contract the plugin implements, by its renderer
    let renderers: Vec<&String> = interfaces
        .exports
        .iter()
        .filter(|export| split(export).is_some_and(|(base, _)| base == RENDERER))
        .collect();
    let renderer = match renderers[..] {
        [renderer] => renderer,
        [] => {
            // Imports from a version of the contract the host does not run say which one the
            // plugin was built for
            let mut unknown: Vec<String> = interfaces
                .imports
                .iter()
                .filter(|import| {
                    import.starts_with(CONTRACT_PREFIX)
                        && !split(import).is_some_and(|(_, version)| {
                            contract_of(&version).is_some() && version <= V1_VERSION
                        })
                })
                .cloned()
                .collect();
            unknown.sort();
            return Err(RuntimeError::with_names(
                ErrorKind::ContractMismatch,
                unknown,
                Some(&format!(
                    "exports no renderer of a contract this host runs ({}, or v1 up to {})",
                    Contract::V0.renderer(),
                    Contract::V1.renderer()
                )),
            ));
        }
        _ => {
            return Err(RuntimeError::new(
                ErrorKind::ContractMismatch,
                "exports the renderer of more than one contract version",
            ));
        }
    };
    let (_, version) = split(renderer).expect("a renderer export has a version");
    let Some(contract) = contract_of(&version) else {
        return Err(RuntimeError::with_names(
            ErrorKind::ContractMismatch,
            vec![renderer.clone()],
            Some(&format!(
                "is built for contract {version}, which this host does not run (v0 0.1.0, or v1 up to {V1_VERSION})"
            )),
        ));
    };
    if contract == Contract::V1 && version > V1_VERSION {
        return Err(RuntimeError::with_names(
            ErrorKind::ContractMismatch,
            vec![renderer.clone()],
            Some(&format!(
                "is built for contract {version}, newer than {V1_VERSION}, the newest this host runs; update the host"
            )),
        ));
    }
    // Every contract name of the component carries that one version
    let mut foreign: Vec<String> = interfaces
        .imports
        .iter()
        .chain(&interfaces.exports)
        .filter(|name| {
            name.starts_with(CONTRACT_PREFIX)
                && split(name).map(|(_, other)| other) != Some(version.clone())
        })
        .cloned()
        .collect();
    if !foreign.is_empty() {
        foreign.sort();
        return Err(RuntimeError::with_names(
            ErrorKind::ContractMismatch,
            foreign,
            Some(&format!(
                "implements contract {} {version} but uses another version of it; every contract name must carry {version}",
                contract.as_str()
            )),
        ));
    }
    let contract = ContractVersion { contract, version };
    if contract.contract == Contract::V0 {
        let optional: Vec<String> = manifest
            .permissions()
            .iter()
            .filter(|permission| !permission.required)
            .map(|permission| permission.name.clone())
            .collect();
        if !optional.is_empty() {
            return Err(RuntimeError::with_names(
                ErrorKind::OptionalNeedsV1,
                optional,
                Some(
                    "contract v0 cannot answer not-granted, so its permissions must be required; build against contract v1",
                ),
            ));
        }
    }
    // A widget with connectors fetches its data in `update`, which arrived with 1.1.0
    if !manifest.connectors().is_empty() {
        let ids: Vec<String> = manifest
            .connectors()
            .iter()
            .map(|connector| connector.id.clone())
            .collect();
        if !contract.has_connectors() {
            return Err(RuntimeError::with_names(
                ErrorKind::ContractTooOld,
                ids,
                Some(&format!(
                    "connectors need contract {CONNECTORS_SINCE} or later, and this plugin is built for {}",
                    contract.version
                )),
            ));
        }
        if interfaces.updater().is_none() {
            return Err(RuntimeError::with_names(
                ErrorKind::MissingUpdater,
                ids,
                Some("a widget that declares connectors must export updater, where it calls them"),
            ));
        }
    }

    // Step 6: every import is a capability the host offers, through a declared permission
    let mut refused = Vec::new();
    let mut undeclared = Vec::new();
    for import in &interfaces.imports {
        if contract.has_connectors() && is_connectors(import) {
            if manifest.connectors().is_empty() {
                undeclared.push("connectors".to_owned());
            }
            continue;
        }
        match by_import(contract.contract, import) {
            None => refused.push(import.clone()),
            Some(permission) if manifest.permission(permission.name).is_none() => {
                undeclared.push(permission.name.to_owned());
            }
            Some(_) => {}
        }
    }
    if !refused.is_empty() {
        refused.sort();
        return Err(RuntimeError::with_names(
            ErrorKind::CapabilityRefused,
            refused,
            None,
        ));
    }
    if !undeclared.is_empty() {
        undeclared.sort();
        undeclared.dedup();
        return Err(RuntimeError::with_names(
            ErrorKind::NotDeclared,
            undeclared,
            Some("the component imports them, but its manifest does not declare them"),
        ));
    }
    Ok(contract)
}

/// Load steps 7–9: the plugin's approval from `origin`, then what to prune and what to link.
pub(crate) fn decide(
    contract: Contract,
    interfaces: &Interfaces,
    manifest: &Manifest,
    origin: &str,
    entry: Option<&Entry>,
) -> Result<Decision, RuntimeError> {
    // Step 7: an approval from this package's origin, holding every required permission,
    // imported or not, because required permissions are approved together (FR-018)
    let id = manifest.id();
    let required: Vec<String> = manifest
        .permissions()
        .iter()
        .filter(|permission| permission.required)
        .map(|permission| permission.name.clone())
        .collect();
    let entry = match entry {
        None => {
            return Err(RuntimeError::with_names(
                ErrorKind::NotGranted,
                required,
                Some(&format!("{id} has no approval; approve it first")),
            ));
        }
        Some(entry) if entry.origin != origin => {
            return Err(RuntimeError::with_names(
                ErrorKind::NotGranted,
                required,
                Some(&format!(
                    "{id} is approved from {}, not from {origin}; approve this package to replace that approval",
                    entry.origin
                )),
            ));
        }
        Some(entry) => entry,
    };
    let missing: Vec<String> = required
        .into_iter()
        .filter(|name| !entry.is_granted(name))
        .collect();
    if !missing.is_empty() {
        return Err(RuntimeError::with_names(
            ErrorKind::NeedsNewApproval,
            missing,
            Some(&format!(
                "{id} {} was approved, and this version requires more; approve it again",
                entry.approved_version
            )),
        ));
    }

    // Step 8: what the record grants that this manifest no longer declares
    let prune = entry
        .granted
        .iter()
        .filter(|name| manifest.permission(name).is_none())
        .cloned()
        .collect();

    // Step 9: each import gets the real interface when granted, and the stand-in when it is an
    // optional permission that is off. Steps 6 and 7 leave no other case; one would be refused.
    let mut plan = LinkPlan {
        contract,
        real: Vec::new(),
        stand_in: Vec::new(),
        connectors: false,
    };
    for import in &interfaces.imports {
        // Step 6 let it through only for a 1.1 plugin that declares connectors
        if is_connectors(import) {
            plan.connectors = true;
            continue;
        }
        let permission = by_import(contract, import);
        let request = permission.and_then(|permission| manifest.permission(permission.name));
        match (permission, request) {
            (Some(permission), Some(_)) if entry.is_granted(permission.name) => {
                plan.real.push(permission.name);
            }
            (Some(permission), Some(request)) if !request.required => {
                plan.stand_in.push(permission.name);
            }
            _ => {
                return Err(RuntimeError::with_names(
                    ErrorKind::NotGranted,
                    vec![import.clone()],
                    Some("neither declared and granted, nor an optional permission that is off"),
                ));
            }
        }
    }
    Ok(Decision { plan, prune })
}

/// The permission named `name`, if the host knows it.
pub(crate) fn by_name(name: &str) -> Option<&'static Permission> {
    REGISTRY.iter().find(|permission| permission.name == name)
}

/// The permission whose interface in `contract` is `import`: in v0 matched exactly; in v1 by
/// name, at a version that has it. Step 5 already checked the version is one the host runs.
pub(crate) fn by_import(contract: Contract, import: &str) -> Option<&'static Permission> {
    REGISTRY.iter().find(|permission| match contract {
        Contract::V0 => permission.v0 == Some(import),
        Contract::V1 => split(import).is_some_and(|(base, version)| {
            base == permission.v1
                && version.major == 1
                && version >= permission.since
                && version <= V1_VERSION
        }),
    })
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

    /// A contract 1.0.0 component: the suite of feature 002 keeps checking the oldest v1.
    fn v1(imports: &[&str]) -> Interfaces {
        interfaces(imports, Some(RENDERER_V1_0))
    }

    const RENDERER_V1_0: &str = "dashboard:widget/renderer@1.0.0";

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
            contract.contract,
            interfaces,
            manifest,
            origin,
            entry(store).as_ref(),
        )
    }

    #[test]
    fn the_renderer_export_sets_the_contract() {
        assert_eq!(
            check_component(&v0(&[]), &manifest(None)).map(|c| c.contract),
            Ok(Contract::V0)
        );
        assert_eq!(
            check_component(&v1(&[]), &manifest(None)).map(|c| c.contract),
            Ok(Contract::V1)
        );
    }

    #[test]
    fn no_renderer_of_a_known_version_is_a_contract_mismatch() {
        for renderer in [
            None,
            Some("dashboard:widget/renderer@1.2.0"),
            Some("dashboard:widget/renderer@2.0.0"),
            Some("dashboard:widget/renderer@0.2.0"),
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
            check_component(&v1(&[LOG_V1]), &manifest(Some(false))).map(|c| c.contract),
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
                stand_in: vec![],
                connectors: false,
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
        assert_eq!(Contract::V1.renderer(), "dashboard:widget/renderer@1.1.0");
        assert_eq!(
            Contract::from_renderer("dashboard:widget/renderer@0.1.0"),
            Some(Contract::V0)
        );
        for v1 in [
            "dashboard:widget/renderer@1.0.0",
            "dashboard:widget/renderer@1.1.0",
        ] {
            assert_eq!(Contract::from_renderer(v1), Some(Contract::V1));
        }
        assert_eq!(Contract::V0.as_str(), "v0");
        assert_eq!(Contract::V1.as_str(), "v1");
    }

    #[test]
    fn another_renderer_version_is_no_contract() {
        for export in [
            "dashboard:widget/renderer@1.2.0",
            "dashboard:widget/renderer@2.0.0",
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
        assert_eq!(log.v0, Some("dashboard:widget/log@0.1.0"));
        assert_eq!(log.v1, "dashboard:widget/log");
        assert_eq!(log.since, semver::Version::new(1, 0, 0));
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
        assert_eq!(by_import(Contract::V1, "dashboard:widget/log@1.2.0"), None);
        assert_eq!(by_import(Contract::V1, "dashboard:widget/log@2.0.0"), None);
    }

    #[test]
    fn an_unknown_name_or_import_matches_nothing() {
        assert_eq!(by_name("telepathy"), None);
        assert_eq!(by_name("Log"), None);
        assert_eq!(by_name(""), None);
        assert_eq!(by_import(Contract::V1, "wasi:filesystem/types@0.2.0"), None);
    }

    const CONNECTORS_V1_1: &str = "dashboard:widget/connectors@1.1.0";
    const LOG_V1_1: &str = "dashboard:widget/log@1.1.0";
    const UPDATER_V1_1: &str = "dashboard:widget/updater@1.1.0";

    /// A WAT component with these `imports` and `exports` (each an empty instance).
    fn component(imports: &[&str], exports: &[&str]) -> Interfaces {
        let mut wat = String::from("(component");
        for import in imports {
            wat.push_str(&format!(" (import \"{import}\" (instance))"));
        }
        for (index, export) in exports.iter().enumerate() {
            wat.push_str(&format!(
                " (instance $e{index}) (export \"{export}\" (instance $e{index}))"
            ));
        }
        wat.push(')');
        let engine = Engine::default();
        let component = Component::new(&engine, wat).expect("the test component compiles");
        Interfaces::of(&engine, &component)
    }

    /// A manifest declaring `catalog/github` and `log` required.
    fn connected_manifest() -> Manifest {
        let manifest = json!({
            "manifest_version": 1,
            "id": "local/probe",
            "version": "1.0.0",
            "author": "A",
            "permissions": [{ "name": "log", "required": true }],
            "login_providers": [],
            "connectors": [{ "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true }]
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    #[test]
    fn a_v1_plugin_of_any_minor_up_to_the_hosts_runs_at_its_version() {
        let old = check_component(
            &component(&[LOG_V1], &[RENDERER_V1_0]),
            &manifest(Some(true)),
        )
        .unwrap();
        assert_eq!(old.contract, Contract::V1);
        assert_eq!(old.version, semver::Version::new(1, 0, 0));
        assert_eq!(old.display(), "v1 (1.0.0)");
        assert!(!old.has_connectors());

        let new = check_component(
            &component(&[LOG_V1_1], &["dashboard:widget/renderer@1.1.0"]),
            &manifest(Some(true)),
        )
        .unwrap();
        assert_eq!(new.version, semver::Version::new(1, 1, 0));
        assert_eq!(new.display(), "v1 (1.1.0)");
        assert!(new.has_connectors());

        let v0 = check_component(&v0(&[LOG_V0]), &manifest(Some(true))).unwrap();
        assert_eq!(v0.display(), "v0");
    }

    #[test]
    fn a_plugin_mixing_minor_versions_is_a_contract_mismatch_naming_both() {
        for (imports, exports) in [
            (&[LOG_V1_1][..], &[RENDERER_V1_0][..]),
            (&[LOG_V1][..], &["dashboard:widget/renderer@1.1.0"][..]),
            (&[][..], &[RENDERER_V1_0, UPDATER_V1_1][..]),
        ] {
            let error = check_component(&component(imports, exports), &manifest(Some(true)))
                .expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::ContractMismatch, "{error}");
            assert!(error.detail().contains("1.0.0"), "{error}");
            assert!(error.detail().contains("1.1.0"), "{error}");
        }
    }

    #[test]
    fn a_plugin_built_for_a_newer_minor_is_a_contract_mismatch_naming_both() {
        let error = check_component(
            &component(
                &["dashboard:widget/log@1.2.0"],
                &["dashboard:widget/renderer@1.2.0"],
            ),
            &manifest(Some(true)),
        )
        .expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::ContractMismatch);
        assert!(error.detail().contains("1.2.0"), "{error}");
        assert!(error.detail().contains("1.1.0"), "{error}");
    }

    #[test]
    fn connectors_need_contract_1_1_and_an_updater() {
        // Built for 1.0.0
        let error = check_component(
            &component(&[LOG_V1], &[RENDERER_V1_0]),
            &connected_manifest(),
        )
        .expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::ContractTooOld, "{error}");
        assert!(error.detail().contains("1.1.0"), "{error}");
        assert_eq!(error.names(), ["catalog/github"]);
        // Built for v0
        let error = check_component(&v0(&[LOG_V0]), &connected_manifest()).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::ContractTooOld, "{error}");
        // 1.1.0 without updater
        let error = check_component(
            &component(&[CONNECTORS_V1_1], &["dashboard:widget/renderer@1.1.0"]),
            &connected_manifest(),
        )
        .expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::MissingUpdater, "{error}");
        // 1.1.0 with updater
        let checked = check_component(
            &component(
                &[LOG_V1_1, CONNECTORS_V1_1],
                &["dashboard:widget/renderer@1.1.0", UPDATER_V1_1],
            ),
            &connected_manifest(),
        )
        .unwrap();
        assert!(checked.has_connectors());
    }

    #[test]
    fn importing_connectors_without_declaring_one_is_not_declared() {
        assert_eq!(
            refusal(check_component(
                &component(
                    &[CONNECTORS_V1_1],
                    &["dashboard:widget/renderer@1.1.0", UPDATER_V1_1]
                ),
                &manifest(None),
            )),
            (ErrorKind::NotDeclared, vec!["connectors".to_owned()])
        );
        // A 1.0.0 component cannot import connectors at all
        let error = check_component(
            &component(&["dashboard:widget/connectors@1.0.0"], &[RENDERER_V1_0]),
            &manifest(None),
        )
        .expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused, "{error}");
    }

    #[test]
    fn generic_http_from_wasi_is_refused() {
        for import in [
            "wasi:http/outgoing-handler@0.2.0",
            "wasi:http/outgoing-handler@0.2.6",
            "wasi:http/types@0.2.0",
            "wasi:http/handler@0.3.0",
            "wasi:http/client@0.3.0",
            "wasi:http/types@0.3.0",
            "wasi:sockets/tcp@0.3.0",
        ] {
            assert_eq!(
                refusal(check_component(
                    &component(&[import], &["dashboard:widget/renderer@1.1.0"]),
                    &manifest(None),
                )),
                (ErrorKind::CapabilityRefused, vec![import.to_owned()]),
                "{import}"
            );
        }
    }

    #[test]
    fn the_registry_has_log_since_1_0_and_connectors_since_1_1() {
        for import in ["dashboard:widget/log@1.0.0", LOG_V1_1] {
            assert_eq!(by_import(Contract::V1, import).map(|p| p.name), Some("log"));
        }
        // Connectors are no permission by name: the manifest declares them under `connectors`
        assert_eq!(by_name("connectors"), None);
        assert_eq!(by_import(Contract::V1, CONNECTORS_V1_1), None);
        assert_eq!(CONNECTORS_SINCE, semver::Version::new(1, 1, 0));
        assert!(is_connectors(CONNECTORS_V1_1));
    }

    #[test]
    fn the_link_plan_says_whether_connectors_are_imported() {
        let interfaces = component(
            &[LOG_V1_1, CONNECTORS_V1_1],
            &["dashboard:widget/renderer@1.1.0", UPDATER_V1_1],
        );
        let manifest = connected_manifest();
        let store = approved(&manifest, false);
        let decision = load(&interfaces, &manifest, &store, ORIGIN).unwrap();
        assert!(decision.plan.connectors);
        assert_eq!(decision.plan.real, ["log"]);
        let decision = load(
            &v1(&[LOG_V1]),
            &self::manifest(Some(true)),
            &approved(&self::manifest(Some(true)), false),
            ORIGIN,
        )
        .unwrap();
        assert!(!decision.plan.connectors);
    }
}
