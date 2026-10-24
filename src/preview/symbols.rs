//! Declarations (functions, types, modules) taken from the names the grammar
//! marks as declared, for the outline next to source previews.
//!
//! Sublime grammars name what a line declares with `entity.name.<kind>`;
//! `ito-syntax` hands those names over with the same parse that gives the
//! colours, so one pass serves both, for every language bat ships a grammar
//! for.

use ito_syntax::Decl;

use super::TocEntry;

/// Deeper nesting is shown at this level.
const MAX_LEVEL: usize = 4;
/// A name longer than this is not one anybody reads in an outline.
const MAX_NAME: usize = 80;
/// Kinds shown with their kind in front; functions go bare.
const TAGGED: [&str; 10] = ["class", "struct", "enum", "union", "trait", "interface", "impl", "namespace", "macro", "type"];
/// Words in front of a name that say what it declares, for the grammars that
/// scope every type name `entity.name.type` (TypeScript, Go, ...).
const KEYWORDS: [(&str, &str); 9] = [
    ("class", "class"),
    ("struct", "struct"),
    ("enum", "enum"),
    ("union", "union"),
    ("trait", "trait"),
    ("interface", "interface"),
    ("namespace", "namespace"),
    ("module", "namespace"),
    ("type", "type"),
];

/// Fed the parse of a file line by line; keeps the first declaration on each
/// line, nested by how far it is indented.
#[derive(Default)]
pub struct Collector {
    /// Indents of the enclosing entries, outermost first.
    indents: Vec<usize>,
    entries: Vec<TocEntry>,
}

impl Collector {
    pub fn feed(&mut self, line_idx: usize, line: &str, decls: &[Decl]) {
        let Some(label) = decls.iter().find_map(|d| entry_label(line, d)) else { return };

        let indent = line.len() - line.trim_start().len();
        while self.indents.last().is_some_and(|&w| w >= indent) {
            self.indents.pop();
        }
        self.indents.push(indent);
        let level = self.indents.len().min(MAX_LEVEL);
        self.entries.push(TocEntry { level: level as u8, label: format!("{}{}", "  ".repeat(level - 1), clip(&label)), line: line_idx });
    }

    /// The entries found in the lines fed so far.
    pub fn so_far(&self) -> Vec<TocEntry> {
        self.entries.clone()
    }

    pub fn finish(self) -> Vec<TocEntry> {
        self.entries
    }
}

/// What the outline says for a declaration, or `None` for one it leaves out.
fn entry_label(line: &str, decl: &Decl) -> Option<String> {
    let name = line.get(decl.range.clone())?.trim();
    if name.is_empty() {
        return None;
    }
    match decl.kind.as_str() {
        // `new Foo()` names the constructor it calls, not one it declares.
        "function" if line[..decl.range.start].trim_end().ends_with("new") => None,
        "function" => Some(name.to_owned()),
        // `[dependencies.serde]` comes in pieces; the line says it whole.
        "table" => {
            let line = line.trim();
            Some(line.rfind(']').map_or(line, |end| &line[..=end]).to_owned())
        }
        "module" => Some(format!("mod {name}")),
        // `entity.name.type.class.ts`, `entity.name.type.go`, ...: told by the
        // word in front. A type name with none is a reference
        // (`let y: Map<string, Foo>`), not a declaration.
        "type" => {
            let tag = keyword_before(&line[..decl.range.start])?;
            Some(format!("{tag} {name}"))
        }
        kind => TAGGED.iter().find(|&&k| k == kind).map(|k| format!("{k} {name}")),
    }
}

/// The declaring word the text in front of a name ends with
/// (`export class `, `type `), if it does.
fn keyword_before(before: &str) -> Option<&'static str> {
    let word = before.trim_end().rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).next()?;
    KEYWORDS.iter().find(|(k, _)| *k == word).map(|&(_, tag)| tag)
}

fn clip(s: &str) -> String {
    match s.char_indices().nth(MAX_NAME) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use ito_syntax::Stream;

    use super::*;

    fn outline(ext: &str, src: &str) -> Vec<(usize, u8, String)> {
        let mut stream = Stream::new(&format!("x.{ext}")).unwrap();
        let mut c = Collector::default();
        for (i, line) in src.lines().enumerate() {
            c.feed(i, line, &stream.line_decls(line).1);
        }
        let entries = c.finish();
        let lines = src.lines().count();
        assert!(entries.iter().all(|e| e.line < lines), "{entries:?}");
        entries.into_iter().map(|e| (e.line, e.level, e.label)).collect()
    }

    fn labels(ext: &str, src: &str) -> Vec<String> {
        outline(ext, src).into_iter().map(|e| e.2).collect()
    }

    #[test]
    fn rust_items_nest_by_indent() {
        let src = "\
pub struct App {
    x: u8,
}

enum E { A }
trait T { fn t(&self); }

impl App {
    pub fn new() -> Self {
        let s: Foo = foo();
        Self::bar()
    }
}

mod tests {
    fn helper() {}
}
fn main() {}
";
        assert_eq!(
            outline("rs", src),
            [
                (0, 1, "struct App".into()),
                (4, 1, "enum E".into()),
                (5, 1, "trait T".into()),
                (7, 1, "impl App".into()),
                (8, 2, "  new".into()),
                (14, 1, "mod tests".into()),
                (15, 2, "  helper".into()),
                (17, 1, "main".into()),
            ]
        );
    }

    #[test]
    fn python_methods_sit_under_their_class() {
        let src = "class Foo(Base):\n    def method(self):\n        call()\n\ndef top():\n    pass\n";
        assert_eq!(outline("py", src), [(0, 1, "class Foo".into()), (1, 2, "  method".into()), (4, 1, "top".into())]);
    }

    #[test]
    fn typescript_skips_calls_and_type_references() {
        let src = "\
const x = new Foo<Bar>();
let y: Map<string, Foo> = z as Baz;
function f(a: Foo): Bar { g(); return x.y(); }
export class C extends D implements I {
  method(x: number) {}
}
interface I { m(): void }
type T = A | B;
enum E { A }
namespace N {}
const arrow = (x) => x;
";
        assert_eq!(labels("ts", src), ["f", "class C", "  method", "interface I", "type T", "enum E", "namespace N", "arrow"]);
        // JSX.Element in a return type is a reference, not a namespace.
        assert_eq!(labels("tsx", "export function App(): JSX.Element { return <Foo /> }\n"), ["App"]);
    }

    #[test]
    fn go_and_c() {
        let go = "package main\n\nfunc main() { f() }\nfunc (r *R) Method() {}\ntype S struct {}\n";
        assert_eq!(labels("go", go), ["main", "Method", "type S"]);
        let c = "#include <stdio.h>\nstruct s { int a; };\nint main(void) {\n    printf(\"x\");\n}\n";
        assert_eq!(labels("c", c), ["struct s", "main"]);
    }

    #[test]
    fn toml_tables() {
        let src = "[package]\nname = \"x\"\n\n[dependencies.serde] # pinned\nversion = \"1\"\n[[bin]]\n";
        assert_eq!(labels("toml", src), ["[package]", "[dependencies.serde]", "[[bin]]"]);
    }

    #[test]
    fn plain_prose_has_no_outline() {
        assert!(labels("rs", "// just a comment\nlet x = 1;\n").is_empty());
        assert!(labels("yaml", "key:\n  nested: 1\n").is_empty());
    }

    #[test]
    fn deep_nesting_is_capped() {
        let src = "mod a {\n mod b {\n  mod c {\n   mod d {\n    mod e {\n     fn f() {}\n}}}}}\n";
        let levels: Vec<u8> = outline("rs", src).into_iter().map(|e| e.1).collect();
        assert_eq!(levels, [1, 2, 3, 4, 4, 4]);
    }
}
