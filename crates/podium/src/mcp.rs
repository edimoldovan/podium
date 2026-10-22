//! How Claude reaches the window. `podium mcp` speaks MCP on stdin and
//! stdout and carries each tool call over a socket to the window, which runs
//! it on the deck on screen; when no window is open, it opens one.
//!
//! The same socket lets a second `podium` hand its file to the window that
//! is already open, rather than opening another.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use eframe::egui;
use serde_json::{Value, json};

use crate::{claude, tools};

/// What reaches the window over the socket.
pub enum Incoming {
    Call { tool: String, args: Value, reply: Sender<Value> },
    /// Another `podium` was started: come forward, with its file if it named one.
    Open(Option<PathBuf>),
}

pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("podium.sock")
}

/// Hands a file to the window already open; false when there is none.
pub fn hand_off(path: Option<&Path>) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path()) else { return false };
    if writeln!(stream, "{}", json!({ "open": path })).is_err() {
        return false;
    }
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).is_ok()
}

/// Listens for calls and hand-offs, waking the window for each.
pub fn listen(ctx: egui::Context) -> Option<Receiver<Incoming>> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).ok()?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (tx, ctx) = (tx.clone(), ctx.clone());
            std::thread::spawn(move || answer(stream, tx, ctx));
        }
    });
    Some(rx)
}

/// One request on one connection: a line of JSON in, a line out.
fn answer(stream: UnixStream, tx: Sender<Incoming>, ctx: egui::Context) {
    let Ok(read) = stream.try_clone() else { return };
    let mut line = String::new();
    if BufReader::new(read).read_line(&mut line).is_err() {
        return;
    }
    let Ok(request) = serde_json::from_str::<Value>(&line) else { return };
    let reply = if let Some(open) = request.get("open") {
        let _ = tx.send(Incoming::Open(open.as_str().map(PathBuf::from)));
        ctx.request_repaint();
        json!({})
    } else {
        let (reply, answered) = channel();
        let tool = request["tool"].as_str().unwrap_or_default().to_string();
        let _ = tx.send(Incoming::Call { tool, args: request["args"].clone(), reply });
        ctx.request_repaint();
        answered
            .recv_timeout(Duration::from_secs(300))
            .unwrap_or_else(|_| tools::reply(Err("Podium did not answer in time.".into())))
    };
    let mut stream = stream;
    let _ = writeln!(stream, "{reply}");
}

/// `podium mcp`: MCP over stdin and stdout.
pub fn shim() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = message.get("id").cloned() else { continue };
        let result = match message["method"].as_str().unwrap_or_default() {
            "initialize" => Ok(json!({
                "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "podium", "version": env!("CARGO_PKG_VERSION")},
                "instructions": claude::GUIDE,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools::list() })),
            "tools/call" => Ok(forward(&message["params"]["name"], &message["params"]["arguments"])),
            _ => Err(json!({"code": -32601, "message": "method not found"})),
        };
        let reply = match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    Ok(())
}

/// Carries a call to the window and brings its answer back.
fn forward(tool: &Value, args: &Value) -> Value {
    let attempt = || -> Result<Value, String> {
        let mut stream = connect_or_start()?;
        writeln!(stream, "{}", json!({"tool": tool, "args": args})).map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str(&line).map_err(|e| e.to_string())
    };
    attempt().unwrap_or_else(|e| tools::reply(Err(format!("Podium could not be reached: {e}"))))
}

/// The open window's socket; a window opened first when there is none.
fn connect_or_start() -> Result<UnixStream, String> {
    if let Ok(stream) = UnixStream::connect(socket_path()) {
        return Ok(stream);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("could not open Podium: {e}"))?;
    for _ in 0..150 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(stream) = UnixStream::connect(socket_path()) {
            return Ok(stream);
        }
    }
    Err("Podium did not open".into())
}
