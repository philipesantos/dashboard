//! The dangerous combination rule (`specs/010-plugin-installation/research.md`, R7; Principle
//! III): whether data a widget reads as a connected account, or from this machine through a local
//! connector, can reach another destination, judged on the grants it would hold after a change,
//! never on the change alone, so a widget cannot ask for little and gain the rest one grant at a
//! time.
//!
//! The rule does not try to prove a flow: a widget's store and memory join everything it reads,
//! so any source beside any other destination is one. The installation dialog, the update dialog
//! and the development tool all ask this one function.

use serde::Serialize;

use crate::CheckedPackage;

/// Where data comes from that should not leave unseen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Source {
    /// Data read as the account connected to this connector, now or by an earlier version that
    /// kept it in its store (`Entry::account_sources`).
    Account { connector: String },
    /// Data read from this machine, at these origins, through local connectors.
    ThisMachine { origins: Vec<String> },
}

/// One source and every destination its data can reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flow {
    pub source: Source,
    /// Origins as serialized (`https://api.github.com`), in the order the connectors declare
    /// them, each once.
    pub destinations: Vec<String>,
}

/// The flows of `package` if it held the connectors `granted` says it would hold, and
/// `remembered`, the account sources its store may still hold. An empty list means no
/// dangerous combination.
pub fn judge(
    package: &CheckedPackage,
    granted: &dyn Fn(&str) -> bool,
    remembered: &[String],
) -> Vec<Flow> {
    let held: Vec<_> = package
        .connectors()
        .iter()
        .filter(|loaded| granted(loaded.id()))
        .collect();
    let mut flows = Vec::new();

    // This machine: a local connector's origin beside any HTTPS one
    let mut here = Vec::new();
    let mut elsewhere = Vec::new();
    for loaded in &held {
        for origin in loaded.connector().destinations() {
            let list = if origin.is_https() {
                &mut elsewhere
            } else {
                &mut here
            };
            push_once(list, origin.serialization());
        }
    }
    if !here.is_empty() && !elsewhere.is_empty() {
        flows.push(Flow {
            source: Source::ThisMachine { origins: here },
            destinations: elsewhere,
        });
    }

    // Each account the widget acts as, and each one its store may hold from before
    let mut sources: Vec<(String, Vec<String>)> = Vec::new();
    for loaded in &held {
        let uses_account = package
            .manifest()
            .connector(loaded.id())
            .is_some_and(|request| request.account);
        if let (true, Some(credential)) = (uses_account, loaded.connector().credential()) {
            let own = credential
                .origins()
                .iter()
                .map(|origin| origin.serialization())
                .collect();
            sources.push((loaded.id().to_owned(), own));
        }
    }
    for connector in remembered {
        if !sources.iter().any(|(id, _)| id == connector) {
            sources.push((connector.clone(), Vec::new()));
        }
    }
    for (connector, own) in sources {
        let mut destinations = Vec::new();
        for loaded in &held {
            for origin in loaded.connector().destinations() {
                let origin = origin.serialization();
                if !own.contains(&origin) {
                    push_once(&mut destinations, origin);
                }
            }
        }
        if !destinations.is_empty() {
            flows.push(Flow {
                source: Source::Account { connector },
                destinations,
            });
        }
    }
    flows
}

fn push_once(list: &mut Vec<String>, item: String) {
    if !list.contains(&item) {
        list.push(item);
    }
}
