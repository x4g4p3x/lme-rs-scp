use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

fn fixture_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sleepstudy.csv")
        .display()
        .to_string()
}

fn send(stdin: &mut ChildStdin, message: Value) {
    serde_json::to_writer(&mut *stdin, &message).expect("serialize protocol message");
    stdin.write_all(b"\n").expect("terminate protocol message");
    stdin.flush().expect("flush protocol message");
}

fn receive(stdout: &mut BufReader<ChildStdout>) -> Value {
    let mut line = String::new();
    stdout.read_line(&mut line).expect("read protocol response");
    assert!(!line.is_empty(), "server closed stdout without a response");
    serde_json::from_str(&line).unwrap_or_else(|error| panic!("invalid response {line:?}: {error}"))
}

fn spawn_server() -> (Child, ChildStdin, BufReader<ChildStdout>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lme-rs-scp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn lme-rs-scp");
    let stdin = child.stdin.take().expect("child stdin");
    let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
    (child, stdin, stdout)
}

#[test]
fn scp_stdio_fit_workflow_returns_context_resource_and_provenance() {
    let (mut child, mut stdin, mut stdout) = spawn_server();

    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "lme-rs-scp-test", "version": "0.1.0"}
            }
        }),
    );
    let initialized = receive(&mut stdout);
    assert_eq!(initialized["id"], 1);
    assert!(initialized.get("result").is_some(), "{initialized:#}");

    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
    );
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    );
    let tools = receive(&mut stdout);
    let listed = tools["result"]["tools"].as_array().expect("tool array");
    assert_eq!(listed.len(), 1, "the harness should expose one workflow");
    assert_eq!(listed[0]["name"], "fit_model");

    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "fit_model",
                "arguments": {
                    "context": {
                        "experiment_id": "experiment:sleepstudy",
                        "run_id": "run:integration",
                        "objective": "Estimate the Days slope"
                    },
                    "model_kind": "lmm",
                    "formula": "Reaction ~ Days + (1 | Subject)",
                    "data_path": fixture_path(),
                    "reml": true
                }
            }
        }),
    );
    let response = receive(&mut stdout);
    let result = &response["result"]["structuredContent"];
    assert_eq!(result["context"]["experiment_id"], "experiment:sleepstudy");
    assert_eq!(result["resource"]["summary"]["model_kind"], "lmm");
    assert_eq!(result["resource"]["summary"]["num_obs"], 180);
    assert_eq!(result["provenance"]["engine"]["name"], "lme-rs");
    assert_eq!(result["provenance"]["engine"]["version"], "0.2.2");
    assert_eq!(result["provenance"]["outcome"]["converged"], true);
    assert_eq!(
        result["provenance"]["dataset"]["sha256"]
            .as_str()
            .expect("SHA-256 digest")
            .len(),
        64
    );
    assert!(result["resource"]["uri"]
        .as_str()
        .expect("resource URI")
        .starts_with("lme-rs://fits/"));

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}
