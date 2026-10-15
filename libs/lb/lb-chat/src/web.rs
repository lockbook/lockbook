//! The web for a model whose provider does not reach it itself: a search
//! against the engine the user set up in `/.agent/search/`, and a page read
//! as text. A download keeps a file from the web for any provider.

use std::time::Duration;

use lb_rs::blocking::Lb;
use reqwest::Url;
use scraper::{ElementRef, Html, Node, Selector};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::wire::ToolSchema;

pub const SEARCH: &str = "web_search";
pub const FETCH: &str = "fetch";
pub const DOWNLOAD: &str = "download";
/// The folder of search engine files, one engine each.
pub const ENGINES: &str = "/.agent/search";

const RESULTS: usize = 8;
const TIMEOUT: Duration = Duration::from_secs(20);
/// Bytes of a page that are downloaded, and of its text given per fetch.
const DOWNLOAD_MAX: usize = 2 * 1024 * 1024;
/// The largest file a download keeps.
const FILE_MAX: usize = 32 * 1024 * 1024;
const PAGE: usize = 12 * 1024;
const AGENT: &str = "Lockbook (+https://lockbook.net)";

/// Where a search goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Engine {
    Brave { key: String },
    SearXng { base_url: String },
}

#[derive(Deserialize)]
struct EngineFile {
    kind: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    base_url: String,
}

/// Names and values: a request's parameters, or its headers.
type Pairs<'a> = Vec<(&'a str, &'a str)>;

/// One thing a search found.
#[derive(Debug, PartialEq)]
struct Found {
    title: String,
    url: String,
    snippet: String,
}

impl Engine {
    /// The first engine, by file name, that the user has set up.
    pub fn load(lb: &Lb) -> Option<Engine> {
        let folder = lb.get_by_path(ENGINES).ok()?;
        let mut files = lb.get_children(&folder.id).ok()?;
        files.sort_by(|a, b| a.name.cmp(&b.name));
        files
            .iter()
            .filter_map(|f| lb.read_document(f.id, false).ok())
            .find_map(|bytes| Engine::parse(&bytes))
    }

    pub fn parse(bytes: &[u8]) -> Option<Engine> {
        let file: EngineFile = serde_json::from_slice(bytes).ok()?;
        match file.kind.as_str() {
            "brave" if !file.api_key.trim().is_empty() => {
                Some(Engine::Brave { key: file.api_key.trim().to_string() })
            }
            "searxng" if !file.base_url.trim().is_empty() => {
                let base_url = file.base_url.trim().trim_end_matches('/').to_string();
                Some(Engine::SearXng { base_url })
            }
            _ => None,
        }
    }

    /// What the search found, as a markdown list: a link, then its snippet.
    pub fn search(&self, query: &str) -> Result<String, String> {
        let count = RESULTS.to_string();
        let (url, params, headers): (String, Pairs, Pairs) = match self {
            Engine::Brave { key } => (
                "https://api.search.brave.com/res/v1/web/search".into(),
                vec![("q", query), ("count", &count)],
                vec![("X-Subscription-Token", key), ("Accept", "application/json")],
            ),
            Engine::SearXng { base_url } => {
                (format!("{base_url}/search"), vec![("q", query), ("format", "json")], vec![])
            }
        };
        let (_, _, body) = get(&url, &params, &headers, DOWNLOAD_MAX)?;
        let body = String::from_utf8_lossy(&body).into_owned();
        let body: Value =
            serde_json::from_str(&body).map_err(|_| "the search engine did not answer in JSON")?;
        let found = self.found(&body);
        if found.is_empty() {
            return Ok("no results".into());
        }
        let lines = found.iter().map(|f| {
            let snippet =
                if f.snippet.is_empty() { String::new() } else { format!("\n  {}", f.snippet) };
            format!("- [{}]({}){snippet}", f.title, f.url)
        });
        Ok(lines.collect::<Vec<_>>().join("\n"))
    }

    fn found(&self, body: &Value) -> Vec<Found> {
        let (results, snippet) = match self {
            Engine::Brave { .. } => (&body["web"]["results"], "description"),
            Engine::SearXng { .. } => (&body["results"], "content"),
        };
        let text = |v: &Value| plain(v.as_str().unwrap_or_default());
        let results = results.as_array().into_iter().flatten();
        results
            .filter(|r| r["url"].is_string())
            .take(RESULTS)
            .map(|r| Found {
                title: text(&r["title"]),
                url: text(&r["url"]),
                snippet: text(&r[snippet]),
            })
            .collect()
    }
}

/// The schemas of the web tools; a search only where an engine is set up.
pub fn schemas(search: bool) -> Vec<ToolSchema> {
    let schema = |name: &str, description: &str, props: Value, required: &[&str]| ToolSchema {
        name: name.into(),
        description: description.into(),
        parameters: json!({
            "type": "object",
            "properties": props,
            "required": required,
            "additionalProperties": false,
        }),
    };
    let fetch = schema(
        FETCH,
        "Read a web page as text. Only an address the user gave, or one a search, a page, or a note contained.",
        json!({
            "url": { "type": "string", "description": "the page's address, starting with http" },
            "start": { "type": "integer", "description": "optional byte of the text to start from, for a long page" },
        }),
        &["url"],
    );
    let search = search.then(|| {
        schema(
            SEARCH,
            "Search the web. Answers with titles, addresses, and snippets; fetch an address to read it.",
            json!({ "query": { "type": "string", "description": "what to search for" } }),
            &["query"],
        )
    });
    search.into_iter().chain([fetch]).collect()
}

/// The schema of the download tool, which every provider gets.
pub fn download_schema() -> ToolSchema {
    ToolSchema {
        name: DOWNLOAD.into(),
        description: "Save a file from the web, such as a PDF, into the vault and answer with its path. Only an address the user gave, or one a search, a page, or a note contained.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "the file's address, starting with http" },
                "to": { "type": "string", "description": "optional full path to save at, name included; the chat's imports folder otherwise" },
            },
            "required": ["url"],
            "additionalProperties": false,
        }),
    }
}

/// The text of the page at `url` from byte `start`, a screenful at a time.
pub fn fetch(url: &str, start: usize) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("{url} is not a web address"));
    }
    let accept = ("Accept", "text/html, text/*;q=0.9, */*;q=0.5");
    let (kind, _, body) = get(url, &[], &[accept], DOWNLOAD_MAX)?;
    let body = String::from_utf8_lossy(&body).into_owned();
    let text = if kind.contains("html") {
        readable(&body, url)
    } else if kind.starts_with("text/") || kind.contains("json") || kind.contains("xml") {
        body
    } else {
        return Err(format!("{url} is {kind}, not text"));
    };
    Ok(page(&text, start))
}

/// `text` from byte `start`, cut at [`PAGE`] with where to go on from.
fn page(text: &str, start: usize) -> String {
    let floor = |mut at: usize| {
        at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    let (from, to) = (floor(start), floor(start.saturating_add(PAGE)));
    match text.len() - to {
        0 => text[from..to].to_string(),
        rest => format!("{}\n({rest} more bytes; fetch again with start={to})", &text[from..to]),
    }
}

/// The file at `url` and a name for it: the one the server gives, else the
/// address's last part, with an extension for its kind when it has none.
pub fn download(url: &str) -> Result<(String, Vec<u8>), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("{url} is not a web address"));
    }
    let (kind, served, bytes) = get(url, &[], &[], FILE_MAX)?;
    if bytes.len() >= FILE_MAX {
        return Err(format!("{url} is larger than {} MB", FILE_MAX / (1024 * 1024)));
    }
    let from_url = Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments()?.next_back().map(str::to_string))
        .map(|last| percent_decode(&last))
        .filter(|last| !last.is_empty());
    let mut name = served.or(from_url).unwrap_or_else(|| "download".into());
    let ext = EXTENSIONS.iter().find(|(mime, _)| kind.starts_with(mime));
    if let Some((_, ext)) = ext.filter(|_| !name.contains('.')) {
        name = format!("{name}.{ext}");
    }
    Ok((name, bytes))
}

/// Kinds a download may come as, and the extension a name without one gets.
const EXTENSIONS: &[(&str, &str)] = &[
    ("application/pdf", "pdf"),
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/svg", "svg"),
    ("text/csv", "csv"),
    ("text/markdown", "md"),
    ("text/html", "html"),
    ("text/plain", "txt"),
    ("application/json", "json"),
];

/// `%20` and its kin read as the characters they stand for.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The name a `Content-Disposition` header gives the file, if any.
fn served_name(header: Option<&str>) -> Option<String> {
    let name = header?
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("filename="))?
        .trim_matches('"')
        .rsplit('/')
        .next()?
        .to_string();
    Some(name).filter(|n| !n.is_empty())
}

/// The kind, served name, and up to `max` bytes of what is at `url`.
fn get(
    url: &str, params: &[(&str, &str)], headers: &[(&str, &str)], max: usize,
) -> Result<(String, Option<String>, Vec<u8>), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mut request = reqwest::Client::new()
            .get(url)
            .query(params)
            .timeout(TIMEOUT)
            .header("User-Agent", AGENT);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let mut resp = request.send().await.map_err(crate::wire::unsent)?;
        if !resp.status().is_success() {
            return Err(format!("{url} answered {}", resp.status()));
        }
        let kind = resp.headers().get("content-type");
        let kind = kind
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_lowercase();
        let served = resp.headers().get("content-disposition");
        let served = served_name(served.and_then(|v| v.to_str().ok()));
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("{url}: {e}"))? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() >= max {
                break;
            }
        }
        Ok((kind, served, bytes))
    })
}

/// Search engines mark matches with tags; a snippet is its words.
fn plain(text: &str) -> String {
    let words = Html::parse_fragment(text)
        .root_element()
        .text()
        .collect::<String>();
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What is around a page's words and not of them.
const CHROME: &[&str] = &[
    "script", "style", "noscript", "nav", "header", "footer", "aside", "svg", "form", "iframe",
    "template", "button", "head",
];
/// Elements that start a line of their own.
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "section",
    "article",
    "main",
    "br",
    "tr",
    "ul",
    "ol",
    "table",
    "blockquote",
    "pre",
    "figure",
    "dl",
    "dt",
    "dd",
];

/// A page's title and words as markdown: headings, list items, and links
/// (made absolute against `base`) survive; scripts, styles, and navigation
/// do not. The page's `main` or `article` is taken where it has one.
fn readable(html: &str, base: &str) -> String {
    let doc = Html::parse_document(html);
    let select = |css: &str| {
        Selector::parse(css)
            .ok()
            .and_then(|s| doc.select(&s).next())
    };
    let base = Url::parse(base).ok();
    let mut out = String::new();
    if let Some(title) = select("title") {
        out.push_str(&format!("# {}\n", title.text().collect::<String>().trim()));
    }
    if let Some(body) = select("main").or(select("article")).or(select("body")) {
        walk(body, base.as_ref(), &mut out);
    }
    // One blank line at most, and no trailing spaces.
    let mut text = String::new();
    let mut blank = false;
    for line in out.lines().map(str::trim) {
        if line.is_empty() {
            blank = !text.is_empty();
            continue;
        }
        text.push_str(if std::mem::take(&mut blank) {
            "\n\n"
        } else if text.is_empty() {
            ""
        } else {
            "\n"
        });
        text.push_str(line);
    }
    text
}

fn walk(el: ElementRef, base: Option<&Url>, out: &mut String) {
    for child in el.children() {
        let Some(el) = ElementRef::wrap(child) else {
            if let Node::Text(text) = child.value() {
                let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !words.is_empty() {
                    let spaced =
                        text.starts_with(char::is_whitespace) && !out.ends_with(['\n', ' ']);
                    out.push_str(if spaced { " " } else { "" });
                    out.push_str(&words);
                    out.push_str(if text.ends_with(char::is_whitespace) { " " } else { "" });
                }
            }
            continue;
        };
        let name = el.value().name();
        if CHROME.contains(&name) {
            continue;
        }
        let heading = name.strip_prefix('h').and_then(|n| n.parse::<usize>().ok());
        match (name, heading) {
            (_, Some(level)) if (1..=6).contains(&level) => {
                out.push_str(&format!("\n\n{} ", "#".repeat(level)));
                walk(el, base, out);
                out.push_str("\n\n");
            }
            ("li", _) => {
                out.push_str("\n- ");
                walk(el, base, out);
            }
            ("a", _) => {
                let href = el.value().attr("href").unwrap_or_default();
                let to = base
                    .and_then(|b| b.join(href).ok())
                    .filter(|u| u.scheme().starts_with("http"));
                let mut label = String::new();
                walk(el, base, &mut label);
                match (to, label.trim()) {
                    (_, "") => {}
                    (Some(to), label) => out.push_str(&format!("[{label}]({to})")),
                    (None, label) => out.push_str(label),
                }
            }
            _ if BLOCKS.contains(&name) => {
                out.push_str("\n\n");
                walk(el, base, out);
                out.push_str("\n\n");
            }
            _ => walk(el, base, out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    #[test]
    fn an_engine_file_names_its_engine() {
        let brave = Engine::parse(br#"{"kind":"brave","api_key":" k "}"#);
        assert_eq!(brave, Some(Engine::Brave { key: "k".into() }));
        let own = Engine::parse(br#"{"kind":"searxng","base_url":"http://box:8888/"}"#);
        assert_eq!(own, Some(Engine::SearXng { base_url: "http://box:8888".into() }));
        for junk in [&br#"{"kind":"brave"}"#[..], br#"{"kind":"bing","api_key":"k"}"#, b"nope"] {
            assert_eq!(Engine::parse(junk), None);
        }
    }

    /// Both engines' answers come out as the same list, their match
    /// marking gone.
    #[test]
    fn both_engines_answer_as_one_list() {
        let brave = json!({ "web": { "results": [
            { "title": "Rust <strong>1.99</strong>", "url": "https://r.test/a", "description": "Out <strong>now</strong>." },
            { "title": "no address" }] } });
        let own =
            json!({ "results": [{ "title": "Rust", "url": "https://r.test/b", "content": "" }] });
        let found = Engine::Brave { key: "k".into() }.found(&brave);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].title.as_str(), found[0].snippet.as_str()), ("Rust 1.99", "Out now."));
        assert_eq!(Engine::SearXng { base_url: "x".into() }.found(&own)[0].url, "https://r.test/b");
    }

    #[test]
    fn a_search_is_a_list_of_links_with_their_snippets() {
        const ANSWER: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
            {\"results\":[{\"title\":\"Rust\",\"url\":\"https://r.test\",\"content\":\"A language.\"},\
            {\"title\":\"Bare\",\"url\":\"https://b.test\",\"content\":\"\"}]}";
        let (base_url, asked) = mock::serve_capturing(ANSWER);
        let found = Engine::SearXng { base_url }.search("rust lang").unwrap();
        assert_eq!(found, "- [Rust](https://r.test)\n  A language.\n- [Bare](https://b.test)");
        drop(asked);
    }

    /// A page keeps its title, headings, lists, and links, with the links
    /// made whole; what is around the words goes.
    #[test]
    fn a_page_reads_as_its_words() {
        let html = "<html><head><title> T </title><style>x{}</style></head><body>\
            <nav><a href='/home'>Home</a></nav><main><h2>Plan</h2><p>Go <a href='/b?x=1'>there</a>\n now.</p>\
            <ul><li>one</li><li>two</li></ul><script>var x;</script></main><footer>f</footer></body></html>";
        let text = readable(html, "https://a.test/dir/page");
        assert_eq!(text, "# T\n\n## Plan\n\nGo [there](https://a.test/b?x=1) now.\n\n- one\n- two");
    }

    /// A long page comes a screenful at a time, each saying where the next
    /// starts, and the screenfuls put together are the page.
    #[test]
    fn a_long_page_comes_in_screenfuls() {
        let text = "é".repeat(PAGE);
        let mut read = String::new();
        let mut start = 0;
        loop {
            let got = page(&text, start);
            let (body, rest) = got.split_once("\n(").unwrap_or((&got, ""));
            read.push_str(body);
            let Some(next) = rest.split("start=").nth(1) else { break };
            start = next.trim_end_matches(')').parse().unwrap();
        }
        assert_eq!(read, text);
    }

    #[test]
    fn a_fetch_reads_a_page_and_refuses_what_is_not_one() {
        const PAGE_HTML: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
            <html><body><p>Hello there.</p></body></html>";
        const IMAGE: &str =
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nConnection: close\r\n\r\nxx";
        assert_eq!(fetch(&mock::serve_once(PAGE_HTML), 0).unwrap(), "Hello there.");
        assert!(
            fetch(&mock::serve_once(IMAGE), 0)
                .unwrap_err()
                .contains("image/png, not text")
        );
        assert!(
            fetch("file:///etc/passwd", 0)
                .unwrap_err()
                .contains("not a web address")
        );
    }

    /// A download is named by the server, else by its address, with an
    /// extension for its kind; one too large is refused.
    #[test]
    fn a_download_keeps_its_bytes_and_a_name() {
        const NAMED: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\n\
            Content-Disposition: attachment; filename=\"Form 1040.pdf\"\r\nConnection: close\r\n\r\n%PDF-1.4 x";
        const BARE: &str =
            "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nConnection: close\r\n\r\n%PDF";
        let (name, bytes) = download(&mock::serve_once(NAMED)).unwrap();
        assert_eq!((name.as_str(), bytes.as_slice()), ("Form 1040.pdf", b"%PDF-1.4 x".as_slice()));
        let url = format!("{}/irs/f1040%20sr", mock::serve_once(BARE));
        assert_eq!(download(&url).unwrap().0, "f1040 sr.pdf");
        let url = format!("{}/irs/f1040sr.PDF", mock::serve_once(BARE));
        assert_eq!(download(&url).unwrap().0, "f1040sr.PDF");
        assert!(
            download("ftp://x/y")
                .unwrap_err()
                .contains("not a web address")
        );
        let huge = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {FILE_MAX}\r\nConnection: close\r\n\r\n{}",
            "x".repeat(FILE_MAX)
        );
        assert!(
            download(&mock::serve_once(huge.leak()))
                .unwrap_err()
                .contains("larger than 32 MB")
        );
    }
}
