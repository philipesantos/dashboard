//! The permission engine (`specs/002-plugin-permissions/research.md`, R5).
//!
//! The registry is every permission this host knows, each tied to the interface that carries it
//! in each contract version. A plugin can import an interface only through a permission its
//! manifest declares and the user granted.

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
    use super::*;

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
