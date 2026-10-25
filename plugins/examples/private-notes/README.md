# private-notes

Example plugin for contract 1.3.0 (`account-widget`): shows the notes of a demonstration service,
read as the user's account. Its development connector, [`connectors/notes.json`](connectors/notes.json)
(`local/notes`), declares the account: the sign-in addresses, the access it asks for (`notes`,
read), and how the host learns the account's name. The widget declares `"account": true` on that
connector and calls it through `accounts`; the host adds the credential to each request itself,
and the widget never sees it.

`notes.test` is not a real service: the runtime's tests run this example end to end against the
test sign-in service (`runtime/tests/common/signin.rs`), through the test network that only the
runtime's own tests can reach (`specs/005-plugin-credentials/research.md`, R15, R16). Without an
account connected the connector still works and the service answers with its public notes.
