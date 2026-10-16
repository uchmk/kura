//! HTML read as a document: the markup is turned into the events Markdown's
//! parser would give, and `markdown::lay_out` sets them the same way. So a
//! page shows headings, lists, tables, quotes and code like a Markdown file
//! does, and gets the same outline.
//!
//! This is a reader, not a browser: no CSS layout, no scripts, no forms.
//! Images show their alt text. The source is one `M` away.
//!
//! Only "HTML -> events" lives here (the language check is passed in), so it
//! can move next to `markdown.rs` into a crate of its own.

use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Tag, TagEnd};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

use super::markdown::{decode_entities, lay_out};
use super::text::find_token;
use super::Doc;

/// A tag, or the attributes of one, longer than this is treated as text.
const MAX_TAG: usize = 16 * 1024;
/// Nesting deeper than this is read flat.
const MAX_DEPTH: usize = 512;

/// Elements whose content is not markup and not shown.
const RAW: &[&str] = &["script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes"];
const VOID: &[&str] =
    &["br", "hr", "img", "input", "wbr", "area", "base", "col", "embed", "link", "meta", "param", "source", "track", "keygen"];
/// Elements whose whole content is left out.
const SKIP: &[&str] = &["template", "svg", "math", "object", "canvas", "video", "audio", "select", "datalist", "noscript"];
/// What may stand in `<head>`; anything else starts the body.
const HEAD_OK: &[&str] = &["title", "meta", "link", "style", "script", "base", "noscript", "template", "head"];
/// Elements `hidden` / `display:none` can take out.
const HIDEABLE: &[&str] = &[
    "div", "span", "section", "article", "nav", "aside", "header", "footer", "ul", "ol", "menu", "table", "form", "figure", "button",
    "dialog", "a",
];
/// Plain boxes: they only separate what is around them.
const BOXES: &[&str] = &[
    "div", "section", "article", "main", "header", "footer", "nav", "aside", "address", "figure", "form", "fieldset", "details", "center",
    "hgroup", "dl", "body", "html", "search", "dialog",
];
/// Where inline code ends if its tag was never closed.
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "td",
    "th",
    "pre",
    "blockquote",
    "dl",
    "dt",
    "dd",
    "hr",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "aside",
    "main",
    "form",
    "fieldset",
    "figure",
    "details",
    "summary",
    "tbody",
    "thead",
    "tfoot",
    "caption",
    "body",
    "html",
    "center",
    "address",
];
const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
const WALLS: &[&str] = &["td", "th", "caption", "table"];
const LANG_PREFIXES: &[&str] = &["language-", "lang-", "highlight-source-", "highlight-"];

type Attrs = Vec<(String, Option<String>)>;
type Events = Vec<(Event<'static>, Range<usize>)>;

/// Lay out an HTML document for a pane `cols` cells wide.
pub fn render(text: &str, cols: u16, theme: &Theme, syntaxes: &SyntaxSet) -> (Doc, bool) {
    let events = events(text, &|l| find_token(syntaxes, l).is_some());
    lay_out(text, &events, cols, theme, syntaxes)
}

/// The document as Markdown's events. Each range is empty and marks where in
/// `text` the event came from; they never go backwards. `knows` says whether
/// a code language can be highlighted.
pub(super) fn events(text: &str, knows: &dyn Fn(&str) -> bool) -> Events {
    let mut conv = Conv::new(knows);
    conv.run(lex(text));
    conv.finish()
}

// ---------------------------------------------------------------- lexing

enum Tok<'a> {
    Text(&'a str, usize),
    Open { name: String, attrs: Attrs, empty: bool, at: usize },
    Close { name: String, at: usize },
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

fn is_ws_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

fn push_text<'a>(toks: &mut Vec<Tok<'a>>, text: &'a str, from: usize, to: usize) {
    if from < to {
        toks.push(Tok::Text(&text[from..to], from));
    }
}

fn lex(text: &str) -> Vec<Tok<'_>> {
    let b = text.as_bytes();
    let mut toks = Vec::new();
    let (mut i, mut start) = (0, 0);
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &text[i..];
        let jump = if let Some(after) = rest.strip_prefix("<!--") {
            Some(after.find("-->").map_or(rest.len(), |e| e + 7))
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            Some(rest.find('>').map_or(rest.len(), |e| e + 1))
        } else {
            None
        };
        if let Some(n) = jump {
            push_text(&mut toks, text, start, i);
            i += n;
            start = i;
            continue;
        }
        match parse_tag(rest) {
            Some((tok, n)) => {
                push_text(&mut toks, text, start, i);
                let raw = match &tok {
                    Tok::Open { name, empty: false, .. } if RAW.contains(&name.as_str()) => Some(name.clone()),
                    _ => None,
                };
                match tok {
                    Tok::Open { name, attrs, empty, .. } => toks.push(Tok::Open { name, attrs, empty, at: i }),
                    Tok::Close { name, .. } => toks.push(Tok::Close { name, at: i }),
                    t => toks.push(t),
                }
                i += n;
                if let Some(name) = raw {
                    i = skip_raw(text, i, &name);
                }
                start = i;
            }
            None => i += 1,
        }
    }
    push_text(&mut toks, text, start, b.len());
    toks
}

/// Where the content of a raw element ends: after its closing tag, or the end.
fn skip_raw(text: &str, from: usize, name: &str) -> usize {
    let b = text.as_bytes();
    let mut j = from;
    while let Some(p) = text[j..].find("</") {
        let at = j + p + 2;
        let end = at + name.len();
        let matches = b.get(at..end).is_some_and(|s| s.eq_ignore_ascii_case(name.as_bytes()));
        if matches && !b.get(end).is_some_and(u8::is_ascii_alphanumeric) {
            return text[end..].find('>').map_or(text.len(), |g| end + g + 1);
        }
        j = at;
    }
    text.len()
}

/// One tag at the start of `rest` (which begins with `<`), and its length.
fn parse_tag(rest: &str) -> Option<(Tok<'static>, usize)> {
    let b = rest.as_bytes();
    let closing = b.get(1) == Some(&b'/');
    let s = if closing { 2 } else { 1 };
    if !b.get(s).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    let mut i = s;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'-' | b':' | b'_')) {
        i += 1;
    }
    let name = rest[s..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut empty = false;
    loop {
        while i < b.len() && is_ws_byte(b[i]) {
            i += 1;
        }
        if i >= b.len() || i > MAX_TAG {
            return None;
        }
        match b[i] {
            b'>' => {
                i += 1;
                break;
            }
            b'/' => {
                if b.get(i + 1) == Some(&b'>') {
                    empty = true;
                    i += 2;
                    break;
                }
                i += 1;
                continue;
            }
            _ => {}
        }
        let ns = i;
        while i < b.len() && !is_ws_byte(b[i]) && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        if i == ns {
            i += 1;
            continue;
        }
        let key = rest[ns..i].to_ascii_lowercase();
        while i < b.len() && is_ws_byte(b[i]) {
            i += 1;
        }
        let mut value = None;
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && is_ws_byte(b[i]) {
                i += 1;
            }
            match b.get(i) {
                Some(&q) if q == b'"' || q == b'\'' => {
                    let from = i + 1;
                    let limit = (from + MAX_TAG).min(b.len());
                    let e = b[from..limit].iter().position(|&c| c == q)?;
                    value = Some(decode_entities(&rest[from..from + e]));
                    i = from + e + 1;
                }
                _ => {
                    let from = i;
                    while i < b.len() && !is_ws_byte(b[i]) && b[i] != b'>' {
                        i += 1;
                    }
                    value = Some(decode_entities(&rest[from..i]));
                }
            }
        }
        if !closing {
            attrs.push((key, value));
        }
    }
    let tok = if closing { Tok::Close { name, at: 0 } } else { Tok::Open { name, attrs, empty, at: 0 } };
    Some((tok, i))
}

fn attr<'a>(attrs: &'a Attrs, key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_deref().unwrap_or(""))
}

fn has(attrs: &Attrs, key: &str) -> bool {
    attrs.iter().any(|(k, _)| k == key)
}

/// The value of a property in the `style` attribute, lowercased.
fn style_value(attrs: &Attrs, prop: &str) -> Option<String> {
    let style = attr(attrs, "style")?;
    style.split(';').find_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(prop)).then(|| v.replace("!important", "").trim().to_ascii_lowercase())
    })
}

fn is_hidden(attrs: &Attrs) -> bool {
    style_value(attrs, "display").is_some_and(|v| v == "none")
}

/// Whether the element and everything in it is left out.
fn skipped(name: &str, attrs: &Attrs) -> bool {
    if SKIP.contains(&name) || (name == "dialog" && !has(attrs, "open")) {
        return true;
    }
    HIDEABLE.contains(&name) && (has(attrs, "hidden") || is_hidden(attrs))
}

fn align_of(attrs: &Attrs) -> Alignment {
    let word = attr(attrs, "align").map(str::to_ascii_lowercase).or_else(|| style_value(attrs, "text-align"));
    match word.as_deref() {
        Some("left") => Alignment::Left,
        Some("center") => Alignment::Center,
        Some("right") => Alignment::Right,
        _ => Alignment::None,
    }
}

// ------------------------------------------------------------- converting

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Inline markup, or a tag that only needs closing.
    Inline,
    Block,
    Para,
    Heading,
    Quote,
    List,
    Item,
    Table,
    Section,
    Row,
    Cell,
    Caption,
    /// A table inside a cell or heading, which is read as lines of text.
    Flat,
    /// Whatever is inside is dropped.
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// Text outside a paragraph opens one.
    Loose,
    /// Inside a paragraph, list item or list.
    Flow,
    /// Inside a heading or a cell: no blocks, only runs of text.
    Tight,
    /// Table structure: text between cells is dropped.
    Drop,
}

struct Frame {
    kind: Kind,
    name: String,
    /// Events that close the frame, in order.
    end: Vec<TagEnd>,
    /// A boundary follows when it closes.
    boundary: bool,
    /// Language hints for a `<pre>` inside.
    hint: Vec<String>,
    // Tables.
    idx: usize,
    cells: usize,
    all_th: bool,
    head: bool,
    rows: usize,
    aligns: Vec<Alignment>,
    align_row: Option<usize>,
}

impl Frame {
    fn new(kind: Kind, name: &str) -> Self {
        Self {
            kind,
            name: name.to_owned(),
            end: Vec::new(),
            boundary: false,
            hint: Vec::new(),
            idx: usize::MAX,
            cells: 0,
            all_th: true,
            head: false,
            rows: 0,
            aligns: Vec::new(),
            align_row: None,
        }
    }
}

enum RawKind {
    Code,
    Pre,
}

/// Text of `<pre>` or inline code being collected.
struct Raw {
    kind: RawKind,
    name: String,
    depth: usize,
    buf: String,
    at: usize,
    first: usize,
    dropped: bool,
    pre_words: Vec<String>,
    code_words: Vec<String>,
    data_langs: Vec<String>,
    outer: Vec<String>,
}

struct Conv<'k> {
    out: Events,
    stack: Vec<Frame>,
    pos: usize,
    /// Whitespace waiting between two pieces; set when it holds a newline.
    gap: Option<bool>,
    /// Something was written since the block started.
    content: bool,
    /// `<br>`s (or a block boundary) waiting to become line breaks.
    brs: usize,
    skip: Option<(String, usize)>,
    head: bool,
    raw: Option<Raw>,
    knows: &'k dyn Fn(&str) -> bool,
}

impl<'k> Conv<'k> {
    fn new(knows: &'k dyn Fn(&str) -> bool) -> Self {
        Self { out: Vec::new(), stack: Vec::new(), pos: 0, gap: None, content: false, brs: 0, skip: None, head: false, raw: None, knows }
    }

    fn finish(mut self) -> Events {
        self.finish_raw();
        while !self.stack.is_empty() {
            self.pop_frame();
        }
        self.out
    }

    fn emit_at(&mut self, e: Event<'static>, at: usize) {
        let p = self.out.last().map_or(0, |l| l.1.end).max(at);
        self.out.push((e, p..p));
    }

    fn emit(&mut self, e: Event<'static>) {
        self.emit_at(e, self.pos);
    }

    fn run(&mut self, toks: Vec<Tok<'_>>) {
        for tok in toks {
            match &tok {
                Tok::Text(_, at) | Tok::Open { at, .. } | Tok::Close { at, .. } => self.pos = self.pos.max(*at),
            }
            if self.raw.is_some() && self.raw_step(&tok) {
                continue;
            }
            match tok {
                Tok::Text(s, at) => {
                    if self.skip.is_none() && !self.head {
                        self.text(s, at);
                    }
                }
                Tok::Open { name, attrs, empty, .. } => {
                    let void = VOID.contains(&name.as_str());
                    if let Some((n, depth)) = self.skip.as_mut() {
                        if *n == name && !empty && !void {
                            *depth += 1;
                        }
                        continue;
                    }
                    if self.head {
                        if HEAD_OK.contains(&name.as_str()) {
                            continue;
                        }
                        self.head = false;
                    }
                    if name == "head" {
                        self.head = true;
                        continue;
                    }
                    if skipped(&name, &attrs) {
                        if !empty && !void {
                            self.skip = Some((name, 1));
                        }
                        continue;
                    }
                    self.open(&name, &attrs, empty);
                    if empty && !void {
                        self.close(&name);
                    }
                }
                Tok::Close { name, .. } => {
                    if let Some((n, depth)) = self.skip.as_mut() {
                        if *n == name {
                            *depth -= 1;
                            if *depth == 0 {
                                self.skip = None;
                            }
                        }
                        continue;
                    }
                    if self.head {
                        if name == "head" {
                            self.head = false;
                        }
                        continue;
                    }
                    self.close(&name);
                }
            }
        }
    }

    // --- context and separators

    fn ctx(&self) -> Ctx {
        for f in self.stack.iter().rev() {
            match f.kind {
                Kind::Cell | Kind::Heading | Kind::Caption => return Ctx::Tight,
                Kind::Item | Kind::List | Kind::Para => return Ctx::Flow,
                Kind::Quote => return Ctx::Loose,
                Kind::Table | Kind::Section | Kind::Row | Kind::Dead => return Ctx::Drop,
                Kind::Inline | Kind::Block | Kind::Flat => {}
            }
        }
        Ctx::Loose
    }

    fn gap_add(&mut self, newline: bool) {
        self.gap = Some(self.gap.unwrap_or(false) || newline);
    }

    /// Get ready to write inline content: open a paragraph if there is none,
    /// and put down the break or space that came before. False if the content
    /// would be dropped.
    fn lead_in(&mut self) -> bool {
        match self.ctx() {
            Ctx::Drop => return false,
            Ctx::Loose => self.open_para("", false),
            _ => {}
        }
        if self.content {
            if self.brs > 0 {
                for _ in 0..self.brs.min(2) {
                    self.emit(Event::HardBreak);
                }
            } else if let Some(newline) = self.gap {
                self.emit(if newline { Event::SoftBreak } else { Event::Text(" ".into()) });
            }
        }
        self.gap = None;
        self.brs = 0;
        self.content = true;
        true
    }

    fn reset_flow(&mut self) {
        self.gap = None;
        self.brs = 0;
        self.content = false;
    }

    fn open_para(&mut self, name: &str, _bold: bool) {
        self.emit(Event::Start(Tag::Paragraph));
        let mut f = Frame::new(Kind::Para, name);
        f.end.push(TagEnd::Paragraph);
        self.stack.push(f);
        self.reset_flow();
    }

    /// Close the paragraph being written, if one is.
    fn close_para(&mut self) -> bool {
        let mut i = self.stack.len();
        while i > 0 && self.stack[i - 1].kind == Kind::Inline {
            i -= 1;
        }
        if i > 0 && self.stack[i - 1].kind == Kind::Para {
            self.pop_to(i - 1);
            true
        } else {
            false
        }
    }

    /// Where one box ends and the next begins.
    fn boundary(&mut self) {
        if self.close_para() {
            return;
        }
        if matches!(self.ctx(), Ctx::Flow | Ctx::Tight) && self.content {
            self.brs = self.brs.max(1);
        }
    }

    // --- frames

    fn pop_frame(&mut self) {
        let Some(f) = self.stack.pop() else { return };
        match f.kind {
            Kind::Row => self.finish_row(&f),
            Kind::Table => {
                if f.idx != usize::MAX {
                    self.out[f.idx].0 = Event::Start(Tag::Table(f.aligns.clone()));
                    self.emit(Event::End(TagEnd::Table));
                }
            }
            _ => {
                for e in f.end.iter().copied() {
                    self.emit(Event::End(e));
                }
            }
        }
        match f.kind {
            Kind::Inline | Kind::Flat | Kind::Section | Kind::Dead => {}
            Kind::Block => {
                if f.boundary {
                    self.boundary();
                }
            }
            _ => self.reset_flow(),
        }
    }

    fn pop_to(&mut self, len: usize) {
        while self.stack.len() > len {
            self.pop_frame();
        }
    }

    /// Close the nearest open element called one of `names`, with whatever is
    /// open inside it, unless a `walls` element comes first.
    fn close_named(&mut self, names: &[&str], walls: &[&str]) {
        for i in (0..self.stack.len()).rev() {
            let n = self.stack[i].name.as_str();
            if names.contains(&n) {
                self.pop_to(i);
                return;
            }
            if walls.contains(&n) {
                return;
            }
        }
    }

    fn inline(&mut self, name: &str, tag: Option<(Tag<'static>, TagEnd)>) {
        let mut f = Frame::new(Kind::Inline, name);
        if let Some((tag, end)) = tag {
            if self.lead_in() {
                self.emit(Event::Start(tag));
                f.end.push(end);
            }
        }
        self.stack.push(f);
    }

    fn block(&mut self, name: &str, attrs: &Attrs) {
        self.boundary();
        let mut f = Frame::new(Kind::Block, name);
        f.boundary = true;
        f.hint = class_words(attrs).into_iter().filter_map(|w| strip_prefix(&w).map(str::to_owned)).collect();
        self.stack.push(f);
    }

    /// A paragraph, or its flat stand-in inside a heading or cell.
    fn para_like(&mut self, name: &str, bold: bool) {
        self.close_para();
        match self.ctx() {
            Ctx::Drop => return,
            Ctx::Tight => {
                self.boundary();
                let mut f = Frame::new(Kind::Block, name);
                f.boundary = true;
                self.stack.push(f);
            }
            _ => self.open_para(name, false),
        }
        if bold {
            self.inline(&format!("{name}-b"), Some((Tag::Strong, TagEnd::Strong)));
        }
    }

    // --- text

    fn text(&mut self, raw: &str, at: usize) {
        if self.ctx() == Ctx::Drop {
            return;
        }
        let mut off = 0;
        for (i, line) in raw.split('\n').enumerate() {
            if i > 0 {
                self.gap_add(true);
            }
            let d = decode_entities(line);
            let t = d.trim_matches(is_ws);
            if t.is_empty() {
                if !d.is_empty() {
                    self.gap_add(false);
                }
            } else {
                if d.starts_with(is_ws) {
                    self.gap_add(false);
                }
                let words = t.split(is_ws).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
                self.pos = self.pos.max(at + off);
                if self.lead_in() {
                    self.emit(Event::Text(words.into()));
                }
                if d.ends_with(is_ws) {
                    self.gap_add(false);
                }
            }
            off += line.len() + 1;
        }
    }

    // --- opening

    fn open(&mut self, name: &str, attrs: &Attrs, empty: bool) {
        if self.stack.len() >= MAX_DEPTH && !VOID.contains(&name) {
            return;
        }
        let table_part = matches!(name, "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "col" | "colgroup");
        if self.ctx() == Ctx::Drop && !table_part {
            return;
        }
        match name {
            "b" | "strong" => self.inline(name, Some((Tag::Strong, TagEnd::Strong))),
            "i" | "em" | "cite" | "dfn" | "var" => self.inline(name, Some((Tag::Emphasis, TagEnd::Emphasis))),
            "s" | "del" | "strike" => self.inline(name, Some((Tag::Strikethrough, TagEnd::Strikethrough))),
            "a" => {
                if let Some(href) = attr(attrs, "href") {
                    let link = Tag::Link { link_type: LinkType::Inline, dest_url: href.to_owned().into(), title: "".into(), id: "".into() };
                    self.inline(name, Some((link, TagEnd::Link)));
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.heading(name),
            "p" | "figcaption" | "legend" => self.para_like(name, false),
            "summary" => self.para_like(name, true),
            "dt" | "dd" => {
                self.close_named(&["dt", "dd"], &["dl", "table", "td", "th", "caption"]);
                self.block(name, attrs);
                if name == "dt" && self.ctx() != Ctx::Drop {
                    self.inline("dt-b", Some((Tag::Strong, TagEnd::Strong)));
                }
            }
            "blockquote" => {
                self.close_para();
                match self.ctx() {
                    Ctx::Drop => {}
                    Ctx::Tight => self.block(name, attrs),
                    _ => {
                        self.emit(Event::Start(Tag::BlockQuote(None)));
                        let mut f = Frame::new(Kind::Quote, name);
                        f.end.push(TagEnd::BlockQuote(None));
                        self.stack.push(f);
                        self.reset_flow();
                    }
                }
            }
            "ul" | "ol" | "menu" | "dir" => {
                self.close_para();
                match self.ctx() {
                    Ctx::Drop => {}
                    Ctx::Tight => self.block(name, attrs),
                    _ => {
                        let ordered = name == "ol";
                        let start = ordered.then(|| attr(attrs, "start").and_then(|s| s.trim().parse().ok()).unwrap_or(1));
                        self.emit(Event::Start(Tag::List(start)));
                        let mut f = Frame::new(Kind::List, name);
                        f.end.push(TagEnd::List(ordered));
                        self.stack.push(f);
                        self.reset_flow();
                    }
                }
            }
            "li" => {
                self.close_named(&["li"], &["ul", "ol", "menu", "dir", "table", "td", "th", "caption"]);
                self.close_para();
                match self.ctx() {
                    Ctx::Drop => {}
                    Ctx::Tight => {
                        self.block(name, attrs);
                        if self.lead_in() {
                            self.emit(Event::Text("• ".into()));
                        }
                    }
                    _ => {
                        self.emit(Event::Start(Tag::Item));
                        let mut f = Frame::new(Kind::Item, name);
                        f.end.push(TagEnd::Item);
                        self.stack.push(f);
                        self.reset_flow();
                    }
                }
            }
            "hr" => {
                self.close_para();
                match self.ctx() {
                    Ctx::Drop => {}
                    Ctx::Tight => self.boundary(),
                    _ => {
                        self.emit(Event::Rule);
                        self.reset_flow();
                    }
                }
            }
            "br" => {
                if self.content && self.ctx() != Ctx::Drop {
                    self.brs = (self.brs + 1).min(2);
                }
            }
            "img" => self.image(attrs),
            "input" => {
                if attr(attrs, "type").is_some_and(|t| t.eq_ignore_ascii_case("checkbox")) && self.lead_in() {
                    self.emit(Event::TaskListMarker(has(attrs, "checked")));
                }
            }
            "code" | "kbd" | "samp" | "tt" if !empty => {
                if self.ctx() != Ctx::Drop {
                    self.raw = Some(Raw::new(RawKind::Code, name, self.pos));
                }
            }
            "pre" | "listing" if !empty => {
                self.close_para();
                let mut raw = Raw::new(RawKind::Pre, name, self.pos);
                raw.pre_words = class_words(attrs);
                raw.data_langs = data_langs(attrs);
                raw.outer = self.stack.iter().rev().flat_map(|f| f.hint.iter().cloned()).collect();
                self.raw = Some(raw);
            }
            "table" => self.table(attrs),
            "caption" => self.caption(),
            "thead" | "tbody" | "tfoot" => {
                self.close_named(&["caption"], &["table"]);
                self.close_named(&["thead", "tbody", "tfoot", "tr", "td", "th"], &["table"]);
                if self.stack.last().is_some_and(|f| f.kind == Kind::Table) {
                    let mut f = Frame::new(Kind::Section, name);
                    f.head = name == "thead";
                    self.stack.push(f);
                }
            }
            "tr" => {
                self.close_named(&["caption"], &["table"]);
                self.close_named(&["tr", "td", "th"], &["table", "thead", "tbody", "tfoot"]);
                match self.stack.last().map(|f| f.kind) {
                    Some(Kind::Table | Kind::Section) => self.open_row(),
                    Some(Kind::Flat) => {
                        self.boundary();
                        self.stack.push(Frame::new(Kind::Inline, "tr"));
                    }
                    _ => {}
                }
            }
            "td" | "th" => {
                self.close_named(&["caption"], &["table"]);
                self.close_named(&["td", "th"], &["tr", "table", "thead", "tbody", "tfoot"]);
                self.cell(name, attrs);
            }
            _ if BOXES.contains(&name) => self.block(name, attrs),
            _ => {}
        }
    }

    fn heading(&mut self, name: &str) {
        self.close_named(HEADINGS, WALLS);
        self.close_para();
        match self.ctx() {
            Ctx::Drop => {}
            Ctx::Tight => {
                self.boundary();
                let mut f = Frame::new(Kind::Block, name);
                f.boundary = true;
                self.stack.push(f);
                self.inline(&format!("{name}-b"), Some((Tag::Strong, TagEnd::Strong)));
            }
            _ => {
                let level = match name {
                    "h1" => HeadingLevel::H1,
                    "h2" => HeadingLevel::H2,
                    "h3" => HeadingLevel::H3,
                    "h4" => HeadingLevel::H4,
                    "h5" => HeadingLevel::H5,
                    _ => HeadingLevel::H6,
                };
                self.emit(Event::Start(Tag::Heading { level, id: None, classes: Vec::new(), attrs: Vec::new() }));
                let mut f = Frame::new(Kind::Heading, name);
                f.end.push(TagEnd::Heading(level));
                self.stack.push(f);
                self.reset_flow();
            }
        }
    }

    fn image(&mut self, attrs: &Attrs) {
        if self.ctx() == Ctx::Drop {
            return;
        }
        let tiny = |k: &str| attr(attrs, k).is_some_and(|v| matches!(v.trim(), "0" | "1" | "0px" | "1px"));
        if attr(attrs, "aria-hidden").is_some_and(|v| v == "true") || is_hidden(attrs) || tiny("width") || tiny("height") {
            return;
        }
        let src = attr(attrs, "src").unwrap_or("");
        let alt = match attr(attrs, "alt") {
            Some(a) if a.trim().is_empty() => return,
            Some(a) => a.split_whitespace().collect::<Vec<_>>().join(" "),
            None if src.is_empty() || src.starts_with("data:") => return,
            None => String::new(),
        };
        if !self.lead_in() {
            return;
        }
        let tag = Tag::Image { link_type: LinkType::Inline, dest_url: src.to_owned().into(), title: "".into(), id: "".into() };
        self.emit(Event::Start(tag));
        if !alt.is_empty() {
            self.emit(Event::Text(alt.into()));
        }
        self.emit(Event::End(TagEnd::Image));
    }

    // --- tables

    fn table(&mut self, attrs: &Attrs) {
        let _ = attrs;
        self.close_para();
        match self.ctx() {
            Ctx::Drop => self.stack.push(Frame::new(Kind::Inline, "table")),
            Ctx::Tight => {
                self.boundary();
                self.stack.push(Frame::new(Kind::Flat, "table"));
            }
            _ => {
                self.stack.push(Frame::new(Kind::Table, "table"));
                self.reset_flow();
            }
        }
    }

    fn caption(&mut self) {
        let top = self.stack.last().map(|f| f.kind);
        match top {
            Some(Kind::Table) if self.stack.last().is_some_and(|f| f.idx == usize::MAX) => {
                self.emit(Event::Start(Tag::Paragraph));
                let mut f = Frame::new(Kind::Caption, "caption");
                f.end.push(TagEnd::Paragraph);
                self.stack.push(f);
                self.reset_flow();
            }
            Some(Kind::Flat) => {
                self.boundary();
                let mut f = Frame::new(Kind::Block, "caption");
                f.boundary = true;
                self.stack.push(f);
            }
            _ => self.stack.push(Frame::new(Kind::Dead, "caption")),
        }
    }

    fn table_frame(&mut self) -> Option<&mut Frame> {
        self.stack.iter_mut().rev().find(|f| f.kind == Kind::Table)
    }

    fn open_row(&mut self) {
        let idx = self.out.len();
        let mut started = false;
        if let Some(t) = self.table_frame() {
            if t.idx == usize::MAX {
                t.idx = idx;
                started = true;
            }
        }
        if started {
            self.emit(Event::Start(Tag::Table(Vec::new())));
        }
        let idx = self.out.len();
        self.emit(Event::Start(Tag::TableRow));
        let mut row = Frame::new(Kind::Row, "tr");
        row.idx = idx;
        self.stack.push(row);
    }

    fn cell(&mut self, name: &str, attrs: &Attrs) {
        match self.stack.last().map(|f| f.kind) {
            Some(Kind::Row) => {}
            Some(Kind::Table | Kind::Section) => self.open_row(),
            Some(Kind::Flat) | Some(Kind::Inline)
                if self.stack.last().is_some_and(|f| f.name == "tr") || self.stack.last().is_some_and(|f| f.kind == Kind::Flat) =>
            {
                self.gap_add(false);
                self.stack.push(Frame::new(Kind::Inline, name));
                return;
            }
            Some(Kind::Inline) if self.stack.last().is_some_and(|f| f.name == "td" || f.name == "th") => {}
            _ => return,
        }
        let th = name == "th";
        let align = align_of(attrs);
        let (col, row_idx) = match self.stack.last_mut() {
            Some(row) if row.kind == Kind::Row => {
                row.cells += 1;
                if !th {
                    row.all_th = false;
                }
                (row.cells - 1, row.idx)
            }
            _ => return,
        };
        if let Some(t) = self.table_frame() {
            if t.aligns.len() <= col {
                t.aligns.resize(col + 1, Alignment::None);
            }
            if align != Alignment::None && t.align_row.is_none_or(|r| r == row_idx) {
                t.aligns[col] = align;
                t.align_row = Some(row_idx);
            }
        }
        self.emit(Event::Start(Tag::TableCell));
        let mut f = Frame::new(Kind::Cell, name);
        if th {
            self.emit(Event::Start(Tag::Strong));
            f.end.push(TagEnd::Strong);
        }
        f.end.push(TagEnd::TableCell);
        self.stack.push(f);
        self.reset_flow();
    }

    fn finish_row(&mut self, row: &Frame) {
        if row.cells == 0 {
            self.out.truncate(row.idx);
            return;
        }
        let in_head = self.stack.last().is_some_and(|f| f.kind == Kind::Section && f.head);
        let first = self.table_frame().is_some_and(|t| t.rows == 0);
        if let Some(t) = self.table_frame() {
            t.rows += 1;
        }
        if in_head || (first && row.all_th) {
            self.out[row.idx].0 = Event::Start(Tag::TableHead);
            self.emit(Event::End(TagEnd::TableHead));
        } else {
            self.emit(Event::End(TagEnd::TableRow));
        }
    }

    // --- closing

    fn close(&mut self, name: &str) {
        match name {
            "br" => {
                if self.content && self.ctx() != Ctx::Drop {
                    self.brs = (self.brs + 1).min(2);
                }
            }
            "table" => self.close_named(&["table"], &[]),
            "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" => self.close_named(&[name], &["table"]),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.close_named(HEADINGS, WALLS),
            _ => self.close_named(&[name], WALLS),
        }
    }

    // --- code

    /// Feed the token to the code being collected. False if it ended the
    /// code and still has to be read on its own.
    fn raw_step(&mut self, tok: &Tok<'_>) -> bool {
        let Some(raw) = self.raw.as_mut() else { return false };
        let pre = matches!(raw.kind, RawKind::Pre);
        match tok {
            Tok::Text(s, at) => {
                let (mut s, mut at) = (*s, *at);
                if raw.buf.is_empty() && !raw.dropped {
                    if let Some(rest) = s.strip_prefix("\r\n").or_else(|| s.strip_prefix('\n')) {
                        at += s.len() - rest.len();
                        s = rest;
                    }
                }
                raw.dropped = true;
                let d = decode_entities(s);
                if !d.is_empty() {
                    if raw.buf.is_empty() {
                        raw.first = at;
                    }
                    raw.buf.push_str(&d);
                }
                true
            }
            Tok::Open { name, attrs, empty, .. } => {
                if *name == raw.name && !empty {
                    raw.depth += 1;
                } else if name == "br" {
                    raw.buf.push(if pre { '\n' } else { ' ' });
                } else if pre {
                    if name == "code" {
                        raw.code_words.extend(class_words(attrs));
                        raw.data_langs.extend(data_langs(attrs));
                    }
                } else if BLOCKS.contains(&name.as_str()) {
                    self.finish_raw();
                    return false;
                }
                true
            }
            Tok::Close { name, .. } => {
                if *name == raw.name {
                    raw.depth -= 1;
                    if raw.depth == 0 {
                        self.finish_raw();
                    }
                    true
                } else if !pre && BLOCKS.contains(&name.as_str()) {
                    self.finish_raw();
                    false
                } else {
                    true
                }
            }
        }
    }

    fn finish_raw(&mut self) {
        let Some(raw) = self.raw.take() else { return };
        match raw.kind {
            RawKind::Code => {
                let text = raw.buf.split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() && self.lead_in() {
                    self.emit(Event::Code(text.into()));
                }
            }
            RawKind::Pre => {
                let code = raw.buf.replace('\r', "");
                let code = code.trim_end();
                if code.is_empty() {
                    return;
                }
                match self.ctx() {
                    Ctx::Drop => {}
                    Ctx::Tight => {
                        for line in code.lines().filter(|l| !l.trim().is_empty()) {
                            self.boundary();
                            if self.lead_in() {
                                self.emit_at(Event::Code(line.trim_end().to_owned().into()), raw.first);
                            }
                        }
                        self.boundary();
                    }
                    _ => {
                        let lang = self.language(&raw);
                        self.emit_at(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang.into()))), raw.at);
                        self.emit_at(Event::Text(format!("{code}\n").into()), raw.first);
                        self.emit_at(Event::End(TagEnd::CodeBlock), raw.first);
                        self.reset_flow();
                    }
                }
            }
        }
    }

    /// The first language hint the highlighter knows.
    fn language(&self, raw: &Raw) -> String {
        let prefixed = raw.pre_words.iter().chain(&raw.code_words).filter_map(|w| strip_prefix(w));
        let plain = raw.pre_words.iter().chain(&raw.code_words).filter(|w| strip_prefix(w).is_none()).map(String::as_str);
        let cands = prefixed.chain(raw.data_langs.iter().map(String::as_str)).chain(raw.outer.iter().map(String::as_str)).chain(plain);
        cands.into_iter().find(|c| !c.is_empty() && (self.knows)(c)).unwrap_or("").to_owned()
    }
}

impl Raw {
    fn new(kind: RawKind, name: &str, at: usize) -> Self {
        Self {
            kind,
            name: name.to_owned(),
            depth: 1,
            buf: String::new(),
            at,
            first: at,
            dropped: false,
            pre_words: Vec::new(),
            code_words: Vec::new(),
            data_langs: Vec::new(),
            outer: Vec::new(),
        }
    }
}

fn class_words(attrs: &Attrs) -> Vec<String> {
    attr(attrs, "class").map(|c| c.split_whitespace().map(str::to_owned).collect()).unwrap_or_default()
}

fn data_langs(attrs: &Attrs) -> Vec<String> {
    ["data-lang", "data-language"].iter().filter_map(|k| attr(attrs, k)).map(|v| v.trim().to_owned()).collect()
}

/// A class word's language, when it has a prefix like `language-`.
fn strip_prefix(word: &str) -> Option<&str> {
    LANG_PREFIXES.iter().find_map(|p| word.strip_prefix(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::LineKind;
    use syntect::highlighting::ThemeSet;

    fn doc_at(html: &str, cols: u16) -> Doc {
        let theme = ThemeSet::load_defaults().themes.remove("base16-ocean.dark").unwrap();
        let syntaxes = SyntaxSet::load_defaults_newlines();
        render(html, cols, &theme, &syntaxes).0
    }

    fn doc(html: &str) -> Doc {
        doc_at(html, 60)
    }

    fn texts(d: &Doc) -> Vec<String> {
        d.lines.iter().map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>().trim_end().to_owned()).collect()
    }

    fn ev(html: &str) -> Events {
        events(html, &|l| l == "rust")
    }

    #[test]
    fn headings_feed_the_outline() {
        let d = doc("<h1>Title</h1><p>text</p><h2>Part <b>two</b></h2><p>more</p>");
        assert_eq!(texts(&d), ["Title", "", "text", "", "Part two", "", "more"]);
        assert!(matches!(d.lines[0].kind, LineKind::Heading(1)));
        assert_eq!(d.toc.iter().map(|t| t.label.trim().to_owned()).collect::<Vec<_>>(), ["Title", "Part two"]);
    }

    #[test]
    fn paragraphs_and_boxes_are_separated() {
        let d = doc("<div>one</div><div>two<br>three</div><p>four</p>five");
        assert_eq!(texts(&d), ["one", "", "two", "three", "", "four", "", "five"]);
    }

    #[test]
    fn whitespace_collapses() {
        let d = doc("<p>a\n   b    c &amp; d&nbsp;e</p>");
        assert_eq!(texts(&d).len(), 1);
        assert!(texts(&d)[0].starts_with("a b c & d"));
    }

    #[test]
    fn japanese_lines_join_without_a_space() {
        let d = doc("<p>日本語の\n文章です</p>");
        assert_eq!(texts(&d), ["日本語の文章です"]);
    }

    #[test]
    fn lists_nest_and_ordered_lists_start_where_told() {
        let d = doc("<ul><li>one<ul><li>inner</li></ul></li><li>two</li></ul><ol start=3><li>c</li><li>d</li></ol>");
        assert_eq!(texts(&d), ["• one", "  ◦ inner", "• two", "", "3. c", "4. d"]);
    }

    #[test]
    fn list_items_without_closing_tags() {
        let d = doc("<ul><li>a<li>b<li>c</ul>");
        assert_eq!(texts(&d), ["• a", "• b", "• c"]);
    }

    #[test]
    fn tables_get_a_head_and_alignment() {
        let d = doc("<table><thead><tr><th>name</th><th align=right>n</th></tr></thead><tbody><tr><td>a</td><td>10</td></tr><tr><td>bb</td><td>2</td></tr></tbody></table>");
        assert_eq!(texts(&d), ["name │  n", "─────┼───", "a    │ 10", "bb   │  2"]);
    }

    #[test]
    fn a_first_row_of_th_is_the_head() {
        let d = doc("<table><tr><th>x</th><th>y</th></tr><tr><td>1</td><td>2</td></tr></table>");
        assert_eq!(texts(&d), ["x │ y", "──┼──", "1 │ 2"]);
    }

    #[test]
    fn empty_tables_and_rows_vanish() {
        let d = doc("<p>a</p><table><tr></tr></table><table></table><p>b</p>");
        assert_eq!(texts(&d), ["a", "", "b"]);
    }

    #[test]
    fn pre_becomes_a_code_block_with_its_source_line() {
        let html = "<p>x</p>\n<pre><code class=\"language-rust\">\nfn main() {}\n</code></pre>";
        let d = doc(html);
        let code: Vec<_> = d.lines.iter().filter(|l| matches!(l.kind, LineKind::Code)).collect();
        assert_eq!(code.len(), 1);
        assert!(code[0].spans.iter().map(|s| s.text.as_str()).collect::<String>().contains("fn main() {}"));
        assert_eq!(code[0].src, 2);
    }

    #[test]
    fn pre_language_comes_from_the_hints() {
        let e = ev("<pre class=\"x\"><code class=\"language-python language-rust\">a</code></pre>");
        assert!(e.iter().any(|(e, _)| matches!(e, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(l))) if &**l == "rust")));
        let e = ev("<div class=\"highlight-source-rust\"><pre>a</pre></div>");
        assert!(e.iter().any(|(e, _)| matches!(e, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(l))) if &**l == "rust")));
    }

    #[test]
    fn inline_code_keeps_its_words() {
        let d = doc("<p>run <code>cargo   build</code> now</p>");
        assert_eq!(texts(&d), ["run cargo build now"]);
    }

    #[test]
    fn entities_are_decoded() {
        let d = doc("<p>a &lt; b &amp;&amp; c &mdash; d &#65;</p>");
        assert!(texts(&d)[0].starts_with("a < b && c — d"));
    }

    #[test]
    fn scripts_styles_and_head_are_not_shown() {
        let d = doc("<html><head><title>T</title><style>p{}</style><script>var a = '<p>x</p>';</script></head><body><p>shown</p><svg><text>no</text></svg><script>no()</script></body></html>");
        assert_eq!(texts(&d), ["shown"]);
    }

    #[test]
    fn a_head_without_its_closing_tag_ends_at_the_first_body_element() {
        let d = doc("<head><meta charset=utf-8><title>t</title><p>shown</p>");
        assert_eq!(texts(&d), ["shown"]);
    }

    #[test]
    fn hidden_things_are_left_out() {
        let d = doc("<div hidden>a</div><div style=\"display: none\">b<div>c</div></div><p>d</p>");
        assert_eq!(texts(&d), ["d"]);
    }

    #[test]
    fn comments_do_not_show() {
        let d = doc("<p>a<!-- <p>no</p> -->b</p><!-- open");
        assert_eq!(texts(&d), ["ab"]);
    }

    #[test]
    fn task_boxes_and_images() {
        let d = doc("<ul><li><input type=checkbox checked> done</li><li><input type=checkbox> todo</li></ul><p><img src=a.png alt=\"A logo\"> <img src=x.png alt=\"\"></p>");
        assert_eq!(texts(&d), ["[x] done", "[ ] todo", "", "[image: A logo]"]);
    }

    #[test]
    fn definition_lists_and_quotes() {
        let d = doc("<dl><dt>term<dd>meaning</dl><blockquote><p>said</p></blockquote>");
        assert_eq!(texts(&d), ["term", "", "meaning", "│", "│ said"]);
    }

    #[test]
    fn broken_input_still_ends() {
        for html in [
            "<p",
            "<p class=\"x",
            "<a href='",
            "<!--",
            "<!",
            "</",
            "<div><div><div>",
            "<table><td>a<table><td>b",
            "<pre>",
            "<code>",
            "<<<>>>",
            "a < b",
        ] {
            for cols in [0, 1, 10, 120] {
                doc_at(html, cols);
            }
        }
    }

    #[test]
    fn deep_nesting_is_cut_off() {
        let html = "<div>".repeat(5000) + "deep" + &"</div>".repeat(5000);
        assert_eq!(texts(&doc(&html)), ["deep"]);
    }

    #[test]
    fn event_positions_never_go_back() {
        let html =
            "<h1>a</h1>\n<ul><li>b</li></ul>\n<pre>\ncode</pre>\n<table><tr><th>x</th></tr><tr><td>1</td></tr></table>\n<p>c<br>d</p>";
        let e = ev(html);
        assert!(e.windows(2).all(|w| w[0].1.end <= w[1].1.start), "{:?}", e.iter().map(|x| x.1.start).collect::<Vec<_>>());
    }

    #[test]
    fn every_start_has_an_end() {
        let e = ev("<table><tr><td><ul><li>a</ul></td></tr></table><blockquote><p>x<b>y</blockquote><h2>z<h3>w");
        let starts = e.iter().filter(|(e, _)| matches!(e, Event::Start(_))).count();
        let ends = e.iter().filter(|(e, _)| matches!(e, Event::End(_))).count();
        assert_eq!(starts, ends);
    }
}
