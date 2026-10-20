use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

const SESSION: &str = include_str!("fixtures/stdio-session.jsonl");

#[test]
fn raw_json_rpc_session_is_mcp_compatible_and_stdout_is_clean() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bitbucket-mcp-server"))
        .env("BITBUCKET_BASE_URL", "http://127.0.0.1:1/bitbucket")
        .env("BITBUCKET_AUTH_TYPE", "token")
        .env("BITBUCKET_TOKEN", "integration-test-token")
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server process starts");

    let mut stdin = child.stdin.take().expect("child stdin is piped");
    stdin
        .write_all(SESSION.as_bytes())
        .expect("raw MCP fixture is written");
    stdin.flush().expect("fixture is flushed");

    let stdout = child.stdout.take().expect("child stdout is piped");
    let mut stdout = BufReader::new(stdout);
    let mut responses = HashMap::new();
    for _ in 0..4 {
        let mut line = String::new();
        stdout.read_line(&mut line).expect("response line is read");
        assert!(!line.is_empty(), "server closed stdout before responding");
        let response: Value = serde_json::from_str(&line).expect("stdout contains only JSON-RPC");
        let id = response["id"].as_i64().expect("response has numeric id");
        responses.insert(id, response);
    }

    let initialize = &responses[&1];
    assert_eq!(initialize["jsonrpc"], "2.0");
    assert_eq!(initialize["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "bitbucket-mcp-server"
    );
    assert!(initialize["result"]["capabilities"]["tools"].is_object());

    let tools = &responses[&2];
    let advertised_tools = tools["result"]["tools"]
        .as_array()
        .expect("tools is an array");
    assert_eq!(advertised_tools.len(), 17);
    assert!(
        advertised_tools
            .iter()
            .any(|tool| tool["name"] == "bitbucket_read_file")
    );
    assert!(
        advertised_tools
            .iter()
            .any(|tool| tool["name"] == "bitbucket_merge_pull_request")
    );
    assert!(tools.get("error").is_none());

    let unknown_tool = &responses[&3];
    assert_eq!(unknown_tool["error"]["code"], -32601);

    let invalid_arguments = &responses[&4];
    assert_eq!(invalid_arguments["error"]["code"], -32602);

    drop(stdin);
    wait_for_exit(&mut child, Duration::from_secs(5));

    let mut remainder = String::new();
    stdout
        .read_to_string(&mut remainder)
        .expect("remaining stdout is readable");
    assert!(
        remainder.trim().is_empty(),
        "unexpected stdout: {remainder}"
    );

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("child stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr is readable");
    assert!(
        stderr.contains("starting Bitbucket MCP server"),
        "missing startup trace: {stderr}"
    );
    assert!(
        stderr.contains("request_id=\"2\""),
        "missing request-id trace: {stderr}"
    );
    assert!(
        stderr.contains("MCP request cancellation received"),
        "missing cancellation trace: {stderr}"
    );
    assert!(
        stderr.contains("mcp_request_completed"),
        "missing metrics hook output: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn sigterm_stops_an_initialized_server_gracefully() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bitbucket-mcp-server"))
        .env("BITBUCKET_BASE_URL", "http://127.0.0.1:1/bitbucket")
        .env("BITBUCKET_AUTH_TYPE", "token")
        .env("BITBUCKET_TOKEN", "integration-test-token")
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server process starts");

    let mut stdin = child.stdin.take().expect("child stdin is piped");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "signal-test", "version": "1" }
            }
        })
    )
    .expect("initialize is written");
    stdin.flush().expect("initialize is flushed");
    let mut stdout = BufReader::new(child.stdout.take().expect("child stdout is piped"));
    let mut response = String::new();
    stdout
        .read_line(&mut response)
        .expect("initialize response is read");
    assert_eq!(
        serde_json::from_str::<Value>(&response).expect("valid JSON")["id"],
        1
    );

    let status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill command executes");
    assert!(status.success());
    wait_for_exit_with_diagnostics(&mut child, Duration::from_secs(5));
}

#[cfg(unix)]
fn wait_for_exit_with_diagnostics(child: &mut std::process::Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("process state is available") {
            assert!(status.success(), "server exited unsuccessfully");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().expect("hung server is terminated");
            child.wait().expect("terminated server is reaped");
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .expect("child stderr is piped")
                .read_to_string(&mut stderr)
                .expect("stderr is readable");
            panic!("server did not stop after SIGTERM: {stderr}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("process state is available") {
            assert!(status.success(), "server exited unsuccessfully");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().expect("hung server is terminated");
            panic!("server did not stop after stdin closed");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
