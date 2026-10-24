//! `storage_cost` runs and prints every line `bench.yml` records
//! (`specs/004-plugin-storage/contracts/cli.md`), with few iterations and a small quota.

#[test]
fn storage_cost_prints_every_measurement() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_storage_cost"))
        .args(["--iterations", "3", "--quota", "65536"])
        .output()
        .expect("storage_cost runs");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for line in [
        "storage_open_us: ",
        "storage_get_us: ",
        "storage_set_us: ",
        "storage_update_100_us: ",
        "render_reading_100_us: ",
    ] {
        assert!(
            stdout.lines().any(|printed| printed.starts_with(line)),
            "no `{line}` in {stdout}"
        );
    }
    let fills: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("file_bytes_at_quota: "))
        .collect();
    assert_eq!(fills.len(), 3, "{stdout}");
    for fill in fills {
        let numbers: Vec<u64> = fill
            .trim_start_matches("file_bytes_at_quota: ")
            .split(' ')
            .map(|number| number.parse().unwrap())
            .collect();
        let [_, used, file] = numbers[..] else {
            panic!("{fill}");
        };
        assert!(used <= 65536 && file <= 5 * 65536 + 64 * 1024, "{fill}");
    }
}
