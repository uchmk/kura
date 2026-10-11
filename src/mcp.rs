//! `kura mcp`: Claude Code reads what the window is showing (Q95, Q96).
//!
//! A windowed program has no standard input to speak MCP on, so the same exe
//! is the bridge:
//!
//! ```text
//! Claude Code ──stdio (JSON-RPC)── kura mcp ──socket / named pipe── kura (the window)
//! ```
//!
//! The window listens on a per-user door ([`address`]); the first window to
//! open it holds it, and the others try again every half minute, so closing
//! that one hands the door on. A request is answered on the UI thread, from
//! the state already in memory: nothing here waits on the disk there.
//!
//! The tools read, `kura_reveal` aside, which moves the cursor and changes no
//! file. The writing ones (`kura_copy`, `kura_move`, `kura_rename`,
//! `kura_trash`; #315, option 1) change nothing until the user has pressed
//! Allow in a box in the window, whose default is Refuse. There is no tool
//! that deletes for good, and no "do not ask again".

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use ito_ipc::{frame, Address, Listener};
use ito_mcp::serde_json::{json, Value};
use ito_mcp::{arg_str, Server, Tool};

/// Bumped when [`Request`] or [`Reply`] change shape, so a window and a
/// `kura mcp` from different versions say so instead of misreading.
const PROTO: u32 = 2;

/// How long `kura mcp` and the door wait for the window to answer. A window
/// that is busy for longer (a modal dialog on Windows) is reported, not waited for.
const ANSWER_WAIT: Duration = Duration::from_secs(5);

/// How long a writing request waits for the user to answer its box. Past it
/// the box no longer does anything (see [`Ask::expired`]).
const DECIDE_WAIT: Duration = Duration::from_secs(120);

/// Most paths one writing request may name; the box lists the first few.
const MAX_PATHS: usize = 500;

/// The door: `\\.\pipe\kura-<user>` on Windows, `$XDG_RUNTIME_DIR/kura/sock`
/// (or `/tmp/kura-<user>/sock`) elsewhere; `KURA_ADDRESS` names another.
pub fn address() -> Address {
    Address::per_user("kura", "KURA_ADDRESS")
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// The folder, the cursor, the selection and the tabs, as JSON.
    State,
    /// Go to this absolute path's folder and put the cursor on it.
    Reveal(PathBuf),
    /// A change to files, which the window asks the user about first.
    Write(Write),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTool {
    Copy,
    Move,
    Rename,
    Trash,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub tool: WriteTool,
    pub paths: Vec<PathBuf>,
    /// The folder to copy or move into, or the new name for a rename.
    pub to: Option<String>,
}

impl Write {
    /// What is wrong with the request on its face, checked off the UI thread
    /// (`symlink_metadata` reads the disk).
    pub fn check(&self) -> Result<(), String> {
        if self.paths.is_empty() {
            return Err("no paths given".into());
        }
        if self.paths.len() > MAX_PATHS {
            return Err(format!("{} paths is more than the {MAX_PATHS} one request may name", self.paths.len()));
        }
        for p in &self.paths {
            if !p.is_absolute() {
                return Err(format!("{} is not an absolute path", p.display()));
            }
            if std::fs::symlink_metadata(p).is_err() {
                return Err(format!("{} does not exist", p.display()));
            }
        }
        let to = self.to.as_deref().unwrap_or("");
        match self.tool {
            WriteTool::Copy | WriteTool::Move => {
                let dir = std::path::Path::new(to);
                if !dir.is_absolute() || !dir.is_dir() {
                    return Err(format!("{to} is not an existing absolute folder path"));
                }
            }
            WriteTool::Rename => {
                if self.paths.len() != 1 {
                    return Err("rename takes exactly one path".into());
                }
                if to.is_empty() || to == "." || to == ".." || to.contains(['/', '\\']) {
                    return Err("the new name must be a file name, not a path".into());
                }
            }
            WriteTool::Trash => {}
        }
        Ok(())
    }

    /// The box's title.
    pub fn title(&self) -> String {
        let n = self.paths.len();
        let items = if n == 1 { "1 item".to_owned() } else { format!("{n} items") };
        match self.tool {
            WriteTool::Copy => format!("Claude asks to copy {items}"),
            WriteTool::Move => format!("Claude asks to move {items}"),
            WriteTool::Rename => "Claude asks to rename an item".to_owned(),
            WriteTool::Trash => format!("Claude asks to move {items} to the trash"),
        }
    }

    /// The box's lines: every path, or the first few and how many more, then
    /// where they go.
    pub fn body(&self) -> Vec<String> {
        const SHOWN: usize = 8;
        let mut out: Vec<String> = self.paths.iter().take(SHOWN).map(|p| p.display().to_string()).collect();
        if self.paths.len() > SHOWN {
            out.push(format!("… and {} more", self.paths.len() - SHOWN));
        }
        let to = self.to.clone().unwrap_or_default();
        match self.tool {
            WriteTool::Copy | WriteTool::Move => out.extend([String::new(), format!("into {to}")]),
            WriteTool::Rename => out.extend([String::new(), format!("to {to}")]),
            WriteTool::Trash => {}
        }
        out
    }
}

/// What the window says: the tool's text, or why it could not.
pub type Reply = Result<String, String>;

/// One request handed to the UI thread, with where its answer goes.
pub struct Ask {
    pub request: Request,
    pub reply: Sender<Reply>,
    /// Set when the one asking stopped waiting: a box for it that is pressed
    /// after that does nothing.
    pub expired: Arc<AtomicBool>,
}

/// Opens the door on a thread of its own and hands each request to the UI
/// thread through the returned channel, waking it with `ctx`.
pub fn listen(ctx: egui::Context) -> Receiver<Ask> {
    let (tx, rx) = bounded::<Ask>(16);
    let at = address();
    let _ = std::thread::Builder::new().name("kura-mcp".into()).spawn(move || {
        let listener = loop {
            match Listener::bind(&at) {
                Ok(l) => break l,
                // Another kura window has it. Try again later, so the door
                // moves on when that window closes.
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => std::thread::sleep(Duration::from_secs(30)),
                // Nothing to report it to that anyone reads: the door is a
                // convenience, and the window works without it.
                Err(_) => return,
            }
        };
        while let Ok(conn) = listener.accept() {
            let tx = tx.clone();
            let ctx = ctx.clone();
            let _ = std::thread::Builder::new().name("kura-mcp-conn".into()).spawn(move || {
                let _ = serve_one(conn, &tx, &ctx);
            });
        }
    });
    rx
}

/// One connection: one request, one reply. A probe that connects and goes
/// away (another window checking whether the door is taken) ends here at the read.
fn serve_one(conn: ito_ipc::Conn, tx: &Sender<Ask>, ctx: &egui::Context) -> io::Result<()> {
    let (mut r, mut w) = conn.split()?;
    let (proto, request): (u32, Request) = frame::read(&mut r)?;
    let reply = if proto != PROTO {
        Err(format!("this kura window speaks version {PROTO} of the door and kura mcp spoke {proto}; run the same kura for both"))
    } else {
        answer(request, tx, ctx)
    };
    frame::write(&mut w, &reply)
}

fn answer(request: Request, tx: &Sender<Ask>, ctx: &egui::Context) -> Reply {
    // Checked here, off the UI thread: a path on a share that stopped
    // answering would hold the window.
    if let Request::Reveal(p) = &request {
        if !p.is_absolute() {
            return Err(format!("{} is not an absolute path", p.display()));
        }
        if std::fs::symlink_metadata(p).is_err() {
            return Err(format!("{} does not exist", p.display()));
        }
    }
    let writes = matches!(request, Request::Write(_));
    if let Request::Write(w) = &request {
        w.check()?;
    }
    let (reply_tx, reply_rx) = bounded(1);
    let expired = Arc::new(AtomicBool::new(false));
    tx.send_timeout(Ask { request, reply: reply_tx, expired: expired.clone() }, ANSWER_WAIT)
        .map_err(|_| "the kura window is busy".to_owned())?;
    ctx.request_repaint();
    let wait = if writes { DECIDE_WAIT } else { ANSWER_WAIT };
    match reply_rx.recv_timeout(wait) {
        Ok(reply) => reply,
        // The box was closed with Esc: that is an answer, and it is no.
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Err("the user closed the box: not done".to_owned()),
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
            expired.store(true, Ordering::Relaxed);
            Err(if writes { "the user did not answer in time: not done" } else { "the kura window did not answer in time" }.to_owned())
        }
    }
}

/// Asks the running window, for `kura mcp`.
fn ask(request: Request) -> Reply {
    let at = address();
    let conn = ito_ipc::connect(&at).map_err(|_| {
        "kura is not running (or its [mcp] enable is false in kura's config.toml); start kura and ask again".to_owned()
    })?;
    let (mut r, mut w) = conn.split().map_err(|e| e.to_string())?;
    frame::write(&mut w, &(PROTO, request)).map_err(|e| format!("could not reach the kura window: {e}"))?;
    frame::read::<_, Reply>(&mut r).map_err(|e| format!("the kura window did not answer: {e}"))?
}

/// The MCP server `kura mcp` runs on its standard input and output.
pub fn server() -> Server {
    Server::new("kura", env!("CARGO_PKG_VERSION"))
        .instructions(
            "kura is the file manager the user has open. kura_state says which folder they are \
             looking at, the file under the cursor and the files they selected; kura_reveal shows \
             them a file. Read the files themselves with your own tools. kura_copy, kura_move, \
             kura_rename and kura_trash change files only after the user presses Allow in a box \
             in their window; they can say no, so tell them what you are asking for and why.",
        )
        .tool(Tool::new(
            "kura_state",
            "What the user's kura window is showing: the current folder, the path under the cursor, \
             the selected paths and the open tabs. Use it when the user says \"this file\", \"here\" \
             or \"the selected files\".",
            |_| ask(Request::State),
        ))
        .tool(
            Tool::new(
                "kura_reveal",
                "Show a file or folder in the user's kura window: go to its folder and put the cursor \
                 on it. Changes no file.",
                |args| ask(Request::Reveal(PathBuf::from(arg_str(args, "path")?))),
            )
            .input(json!({
                "type": "object",
                "properties": { "path": { "type": "string", "description": "An absolute path" } },
                "required": ["path"]
            })),
        )
        .tool(
            Tool::new(
                "kura_copy",
                "Ask to copy files or folders into a folder. The user is asked in their kura window and \
                 nothing is copied unless they press Allow; the copy then runs as one of kura's jobs.",
                |args| write(WriteTool::Copy, args),
            )
            .input(write_schema(true, "The absolute folder to copy into")),
        )
        .tool(
            Tool::new(
                "kura_move",
                "Ask to move files or folders into a folder. The user is asked in their kura window and \
                 nothing is moved unless they press Allow.",
                |args| write(WriteTool::Move, args),
            )
            .input(write_schema(true, "The absolute folder to move into")),
        )
        .tool(
            Tool::new(
                "kura_rename",
                "Ask to rename one file or folder, keeping it where it is. The user is asked in their \
                 kura window and nothing is renamed unless they press Allow.",
                |args| write(WriteTool::Rename, args),
            )
            .input(json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "The absolute path to rename" },
                    "new_name": { "type": "string", "description": "The new file name, without a folder" }
                },
                "required": ["path", "new_name"]
            })),
        )
        .tool(
            Tool::new(
                "kura_trash",
                "Ask to move files or folders to the trash (they can be taken back out). The user is \
                 asked in their kura window and nothing is trashed unless they press Allow. There is \
                 no tool that deletes for good.",
                |args| write(WriteTool::Trash, args),
            )
            .input(write_schema(false, "")),
        )
}

/// A writing tool's call, as the request the window is asked about.
fn write(tool: WriteTool, args: &Value) -> Reply {
    let paths: Vec<PathBuf> = match tool {
        WriteTool::Rename => vec![PathBuf::from(arg_str(args, "path")?)],
        _ => args
            .get("paths")
            .and_then(Value::as_array)
            .ok_or("paths must be a list of absolute paths")?
            .iter()
            .map(|v| v.as_str().map(PathBuf::from).ok_or("paths must be strings"))
            .collect::<Result<_, _>>()?,
    };
    let to = match tool {
        WriteTool::Rename => Some(arg_str(args, "new_name")?.to_owned()),
        WriteTool::Copy | WriteTool::Move => Some(arg_str(args, "to")?.to_owned()),
        WriteTool::Trash => None,
    };
    ask(Request::Write(Write { tool, paths, to }))
}

fn write_schema(has_to: bool, to_doc: &str) -> Value {
    let mut props = json!({ "paths": { "type": "array", "items": { "type": "string" }, "description": "Absolute paths" } });
    let mut need = vec!["paths"];
    if has_to {
        props["to"] = json!({ "type": "string", "description": to_doc });
        need.push("to");
    }
    json!({ "type": "object", "properties": props, "required": need })
}

/// `kura mcp`: serve until the client closes standard input.
pub fn run() -> ! {
    let stdin = io::stdin().lock();
    let stdout = io::stdout().lock();
    let code = match server().serve(stdin, stdout) {
        Ok(()) => 0,
        Err(_) => 1,
    };
    std::process::exit(code)
}

/// The window's answer to `kura_state`, as pretty JSON.
pub fn state_json(app: &crate::app::App, overlay: &str, view: &str) -> String {
    /// A selection of every file in a big folder is not worth the tokens.
    const MAX_SELECTED: usize = 500;
    let tab = app.tab();
    let selected: Vec<String> = tab.selected.iter().take(MAX_SELECTED).map(|p| p.display().to_string()).collect();
    let tabs: Vec<Value> = app
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| json!({ "cwd": t.cwd.display().to_string(), "active": i == app.active }))
        .collect();
    let v = json!({
        "cwd": tab.cwd.display().to_string(),
        "hovered": tab.current.hovered().map(|e| e.path.display().to_string()),
        "selected": selected,
        "selected_count": tab.selected.len(),
        "tabs": tabs,
        "view": view,
        "overlay": overlay,
    });
    ito_mcp::serde_json::to_string_pretty(&v).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &Server, name: &str, args: Value) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } });
        let out = server.handle(&line.to_string()).expect("a call is answered");
        ito_mcp::serde_json::from_str(&out).unwrap()
    }

    /// The tools Claude Code is offered, and that `kura_reveal` asks for a path.
    #[test]
    fn lists_the_tools() {
        let out = server().handle(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let v: Value = ito_mcp::serde_json::from_str(&out).unwrap();
        let names: Vec<&str> = v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["kura_state", "kura_reveal", "kura_copy", "kura_move", "kura_rename", "kura_trash"]);
        // No tool deletes for good.
        assert!(!names.iter().any(|n| n.contains("delete")));
        assert_eq!(v["result"]["tools"][1]["inputSchema"]["required"][0], "path");
    }

    /// A writing request is turned away on its face before the window is asked.
    #[test]
    fn a_bad_write_is_refused_before_any_box() {
        let dir = crate::util::test_dir("mcp-write-check");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let w = |tool, paths: Vec<PathBuf>, to: Option<&str>| Write { tool, paths, to: to.map(str::to_owned) };
        let ok = dir.display().to_string();
        assert!(w(WriteTool::Trash, vec![file.clone()], None).check().is_ok());
        assert!(w(WriteTool::Copy, vec![file.clone()], Some(&ok)).check().is_ok());
        assert!(w(WriteTool::Rename, vec![file.clone()], Some("b.txt")).check().is_ok());
        assert!(w(WriteTool::Trash, vec![], None).check().is_err(), "nothing named");
        assert!(w(WriteTool::Trash, vec![PathBuf::from("a.txt")], None).check().is_err(), "relative");
        assert!(w(WriteTool::Trash, vec![dir.join("none")], None).check().is_err(), "missing");
        assert!(w(WriteTool::Copy, vec![file.clone()], Some("rel")).check().is_err(), "folder is relative");
        assert!(w(WriteTool::Move, vec![file.clone()], Some(&file.display().to_string())).check().is_err(), "folder is a file");
        assert!(w(WriteTool::Rename, vec![file.clone()], Some("../b")).check().is_err(), "a path, not a name");
        assert!(w(WriteTool::Rename, vec![file.clone(), file.clone()], Some("b")).check().is_err(), "two paths");
    }

    /// The box names the tool and the paths, and says how many it leaves out.
    #[test]
    fn the_box_lists_the_first_paths_and_the_count() {
        let paths: Vec<PathBuf> = (0..12).map(|i| PathBuf::from(format!("/x/{i}"))).collect();
        let w = Write { tool: WriteTool::Trash, paths, to: None };
        assert_eq!(w.title(), "Claude asks to move 12 items to the trash");
        let body = w.body();
        assert_eq!(body.len(), 9);
        assert_eq!(body[8], "… and 4 more");
    }

    /// With no window behind the door, the tool says so as a tool error the
    /// model can read, not a protocol error.
    #[test]
    fn no_window_is_a_tool_error() {
        let dir = crate::util::test_dir("mcp-no-window");
        std::fs::create_dir_all(&dir).unwrap();
        // No other test reads it: `address` is called by the tools only.
        std::env::set_var("KURA_ADDRESS", dir.join("sock"));
        let v = call(&server(), "kura_state", json!({}));
        std::env::remove_var("KURA_ADDRESS");
        assert_eq!(v["result"]["isError"], true);
        assert!(v["result"]["content"][0]["text"].as_str().unwrap().contains("kura is not running"));
    }

    /// The door end to end, without a window: a fake UI thread answers what
    /// comes down the channel, and a relative path never reaches it.
    #[cfg(unix)]
    #[test]
    fn the_door_carries_a_request_and_its_reply() {
        let dir = crate::util::test_dir("mcp-door");
        std::fs::create_dir_all(&dir).unwrap();
        let at = Address(dir.join("sock"));
        let listener = Listener::bind(&at).unwrap();
        let (tx, rx) = bounded::<Ask>(1);
        std::thread::spawn(move || {
            while let Ok(ask) = rx.recv() {
                let _ = ask.reply.send(Ok(format!("{:?}", ask.request)));
            }
        });
        let ctx = egui::Context::default();
        std::thread::spawn(move || {
            while let Ok(conn) = listener.accept() {
                let _ = serve_one(conn, &tx, &ctx);
            }
        });
        let send = |req: Request| -> Reply {
            let (mut r, mut w) = ito_ipc::connect(&at).unwrap().split().unwrap();
            frame::write(&mut w, &(PROTO, req)).unwrap();
            frame::read(&mut r).unwrap()
        };
        assert_eq!(send(Request::State), Ok("State".into()));
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(send(Request::Reveal(file.clone())), Ok(format!("Reveal({file:?})")));
        assert!(send(Request::Reveal("a.txt".into())).unwrap_err().contains("not an absolute path"));
        assert!(send(Request::Reveal(dir.join("gone"))).unwrap_err().contains("does not exist"));
    }
}
