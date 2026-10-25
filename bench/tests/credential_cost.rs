//! `credential_cost` runs and prints every line `bench.yml` records, with few iterations.

#[test]
fn credential_cost_prints_every_measurement() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_credential_cost"))
        .args(["--iterations", "3"])
        .output()
        .expect("credential_cost runs");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for line in [
        "request_us: ",
        "request_with_credential_us: ",
        "credential_added_us: ",
        "vault_first_read_us: ",
        "vault_read_us: ",
    ] {
        assert!(
            stdout.lines().any(|printed| printed.starts_with(line)),
            "no `{line}` in {stdout}"
        );
    }
}
