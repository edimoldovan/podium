//! Claude's way in: `podium mcp` on stdin and stdout, carried over the
//! socket to the window, which runs the call on its deck.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use eframe::egui;
use podium::deck::Deck;
use podium::mcp::{self, Incoming};
use podium::session::Session;
use podium::tools;
use serde_json::{Value, json};

#[test]
fn a_call_reaches_the_window_and_its_answer_comes_back() {
    let dir = std::env::temp_dir().join(format!("podium-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: set before any thread of this test binary reads it.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &dir) };

    // The window's side: the socket, and calls run on a deck.
    let rx = mcp::listen(egui::Context::default()).expect("the socket");
    std::thread::spawn(move || {
        let mut session = Session::new(Deck::new(), None);
        for incoming in rx {
            if let Incoming::Call { tool, args, reply } = incoming {
                let _ = reply.send(tools::reply(tools::call(&mut session, &tool, &args)));
            }
        }
    });

    let mut shim = Command::new(env!("CARGO_BIN_EXE_podium"))
        .arg("mcp")
        .env("XDG_RUNTIME_DIR", &dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = shim.stdin.take().unwrap();
    let mut output = BufReader::new(shim.stdout.take().unwrap());
    let mut ask = |message: Value| -> Value {
        writeln!(input, "{message}").unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };

    let hello = ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}));
    assert_eq!(hello["result"]["serverInfo"]["name"], "podium");
    let listed = ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert!(listed["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "podium_page_add"));
    let added = ask(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "podium_page_add", "arguments": {"type": "detail", "title": "Demand doubles"}}}));
    assert_eq!(added["result"]["isError"], false, "{added}");
    let deck = ask(json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "podium_deck", "arguments": {}}}));
    assert!(deck["result"]["content"][0]["text"].as_str().unwrap().contains("Demand doubles"));
    let look = ask(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "podium_look", "arguments": {"page": 3}}}));
    assert_eq!(look["result"]["content"][1]["mimeType"], "image/png");
    drop(input);
    shim.wait().unwrap();
}
