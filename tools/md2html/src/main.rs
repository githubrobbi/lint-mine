// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025-2026 Sky, LLC.

//! `md2html`: one Markdown file to one self-contained HTML page.
//!
//! Forked from iv's `iv-handbook` (same owner, MIT OR Apache-2.0), which
//! itself came from docenta's `handbook-html`. Standard library only: no
//! Markdown crate, no network, nothing to vendor, so it builds anywhere
//! a Rust toolchain does (macOS, Ubuntu).
//!
//! It renders a deliberate subset - headings, paragraphs, tables, fenced
//! code, lists, rules, links, bold, italic and inline code. A construct it
//! does not know appears verbatim, which is visible rather than silent.
//! Known gaps: block quotes inside list items, and bold-italic.
//!
//! A chapter called "Glossary" (a table of term | meaning) becomes live:
//! every term's first mention per chapter is a dotted link that explains
//! itself on hover and jumps to the entry.
//!
//! Usage: `md2html <in.md> [out.html] [--large]`. Without `out.html` the
//! page is written beside the Markdown with the extension swapped.
//! `--large` adds the larger type used for cards read at half a screen.

extern crate alloc;

use alloc::collections::BTreeMap;
use core::fmt::Write as _;
use std::path::PathBuf;

mod markdown;
mod page;

use markdown::{Heading, Term, callout_kind, cells, esc, glossary, heading, inline, list_item};
use page::{Meta, page};

fn main() {
    let mut large = false;
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--large" => large = true,
            "-h" | "--help" => {
                println!("usage: md2html <in.md> [out.html] [--large]");
                return;
            }
            "-V" | "--version" => {
                println!("md2html {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let Some(source) = paths.first().cloned() else {
        eprintln!("usage: md2html <in.md> [out.html] [--large]");
        std::process::exit(64);
    };
    let out = paths
        .get(1)
        .cloned()
        .unwrap_or_else(|| source.with_extension("html"));
    let text = std::fs::read_to_string(&source)
        .unwrap_or_else(|error| panic!("reading {}: {error}", source.display()));
    let terms = glossary(&text);
    let name = source
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let title = text
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map_or_else(|| name.clone(), |title| title.trim().replace('`', ""));
    let meta = Meta {
        title,
        source: name,
        large,
    };
    let html = render(&text, &terms, &meta);
    std::fs::write(&out, &html)
        .unwrap_or_else(|error| panic!("writing {}: {error}", out.display()));
    println!(
        "md2html: {} -> {} ({} bytes, {} glossary terms)",
        source.display(),
        out.display(),
        html.len(),
        terms.len()
    );
}

/// One searchable section: where it is, what it is called, which
/// chapter holds it, and its words.
pub(crate) struct Section {
    /// The anchor.
    pub(crate) id: String,
    /// The heading.
    pub(crate) title: String,
    /// The chapter's heading.
    pub(crate) chapter: String,
    /// The plain words, tags stripped.
    pub(crate) text: String,
}

/// A chapter's number and anchor, from a pass over the headings before
/// rendering, so the reading guide can point at chapters it names.
pub(crate) fn chapters(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(heading)
        .filter(|title| title.level == 2)
        .filter_map(|title| {
            let number = title
                .text
                .split_once('.')
                .and_then(|(head, _rest)| head.trim().parse::<u32>().ok())?;
            Some((number, title.id))
        })
        .collect()
}

/// Rendered HTML back to words, for the search index.
pub(crate) fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            other if !in_tag => out.push(other),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Today as "19 September 2026", from the clock alone (the civil
/// date from days since the epoch, the algorithm every calendar
/// library uses).
pub(crate) fn today() -> String {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let name = usize::try_from(month - 1)
        .ok()
        .and_then(|index| NAMES.get(index))
        .copied()
        .unwrap_or("");
    format!("{day} {name} {year}")
}

/// The page being built: what is written, what is still open, which
/// terms have been linked, and the search index growing beside it.
#[derive(Default)]
pub(crate) struct Body {
    /// The HTML so far.
    pub(crate) out: String,
    /// The contents rail, nested one level.
    pub(crate) contents: String,
    /// Whether a chapter's sub-list is open in the rail.
    sub_open: bool,
    /// The paragraph still being gathered.
    paragraph: String,
    /// The list still open, if any.
    list: Option<&'static str>,
    /// The callout still open: its kind.
    callout: Option<String>,
    /// Terms already linked in THIS chapter.
    seen: BTreeMap<String, usize>,
    /// Every term mention in the book, for unique anchors.
    pub(crate) minted: BTreeMap<String, usize>,
    /// The searchable sections.
    pub(crate) sections: Vec<Section>,
    /// The chapter being rendered.
    pub(crate) chapter: String,
    /// Inside "How to read this": its list becomes cards.
    guide: bool,
    /// The guide bullet still being gathered, continuation lines and all.
    guide_item: String,
    /// The chapter anchors, by number, for the guide's cards.
    numbered: Vec<(u32, String)>,
    /// The first paragraph after the title is the lead.
    lead_pending: bool,
}

impl Body {
    /// Closes whatever block is open.
    fn flush(&mut self, terms: &[Term]) {
        if !self.guide_item.is_empty() {
            let item = self.guide_item.clone();
            self.guide_item.clear();
            self.guide_card(&item);
        }
        if self.guide && !self.paragraph.is_empty() {
            // The first paragraph after the guide's bullets ends the grid.
            self.end_guide();
        }
        if let Some(kind) = self.list.take() {
            let _written = write!(self.out, "</{kind}>");
        }
        if !self.paragraph.is_empty() {
            let rendered = self.inline(&self.paragraph.clone(), terms);
            self.remember(&rendered);
            if self.lead_pending {
                // The lead closes the hero the title opened.
                self.lead_pending = false;
                let _written = write!(self.out, "<p class=\"lead\">{rendered}</p></header>");
            } else {
                let _written = write!(self.out, "<p>{rendered}</p>");
            }
            self.paragraph.clear();
        }
        if self.callout.take().is_some() {
            self.out.push_str("</div></aside>");
        }
    }

    /// Closes the reading guide's card grid, once.
    fn end_guide(&mut self) {
        if self.guide {
            self.out.push_str("</div>");
            self.guide = false;
        }
    }

    /// Inline markup with this page's term bookkeeping.
    fn inline(&mut self, text: &str, terms: &[Term]) -> String {
        inline(text, terms, &mut self.seen, &mut self.minted)
    }

    /// Adds rendered words to the current section's search text.
    fn remember(&mut self, rendered: &str) {
        if let Some(section) = self.sections.last_mut() {
            section.text.push(' ');
            section.text.push_str(&plain(rendered));
        }
    }

    /// A heading: into the article with its anchor, into the rail,
    /// and a new section for the search index.
    fn heading(&mut self, title: &Heading<'_>) {
        let level = title.level;
        let bare = title.text.replace('`', "");
        let text = esc(&bare);
        let shown = inline(title.text, &[], &mut BTreeMap::new(), &mut BTreeMap::new());
        if level == 2 {
            self.seen.clear();
            title.text.clone_into(&mut self.chapter);
            if self.sub_open {
                self.contents.push_str("</ol></li>");
            }
            let _written = write!(
                self.contents,
                "<li><a href=\"#{}\">{text}</a><ol class=\"sub\">",
                title.id
            );
            self.sub_open = true;
        } else if level == 3 && self.sub_open {
            let _written = write!(
                self.contents,
                "<li><a href=\"#{}\">{text}</a></li>",
                title.id
            );
        }
        if level == 1 {
            let _written = write!(self.out, "<h1>{shown}</h1>");
            self.lead_pending = true;
            return;
        }
        if level <= 3 {
            self.sections.push(Section {
                id: title.id.clone(),
                title: bare,
                chapter: self.chapter.clone(),
                text: String::new(),
            });
        }
        let _written = write!(
            self.out,
            "<h{level} id=\"{}\">{shown}<a class=\"anchor\" href=\"#{}\" aria-label=\"Link to this section\">#</a></h{level}>",
            title.id, title.id
        );
    }

    /// A reading-guide bullet as a card: the bold lead names the
    /// chapters, the rest says what they are for. No glossary links inside:
    /// the card is a link, and a link inside a link is not HTML (the browser
    /// closed the card at the first one, 2026-10-08).
    fn guide_card(&mut self, item: &str) {
        let (lead, rest) = item
            .strip_prefix("**")
            .and_then(|tail| tail.split_once("**"))
            .unwrap_or(("", item));
        let first = lead
            .split(|ch: char| !ch.is_ascii_digit())
            .find_map(|piece| piece.parse::<u32>().ok());
        let href = first
            .and_then(|number| {
                self.numbered
                    .iter()
                    .find(|(candidate, _id)| *candidate == number)
                    .map(|(_number, id)| id.clone())
            })
            .unwrap_or_default();
        let said = self.inline(rest.trim(), &[]);
        let _written = write!(
            self.out,
            "<a class=\"card\" href=\"#{href}\"><span class=\"k\">{}</span><span>{said}</span></a>",
            esc(lead)
        );
    }
}

impl Body {
    /// A fence line opens or closes a code block; returns whether the
    /// line was one.
    fn fence(&mut self, line: &str, terms: &[Term], in_code: &mut bool) -> bool {
        let Some(rest) = line.trim_start().strip_prefix("```") else {
            return false;
        };
        if *in_code {
            self.out.push_str("</code></pre></div>");
        } else {
            self.flush(terms);
            self.end_guide();
            let lang = rest.trim();
            let label = if lang.is_empty() {
                String::new()
            } else {
                format!("<span class=\"lang\">{}</span>", esc(lang))
            };
            let named = if lang.is_empty() { "" } else { " named" };
            let _written = write!(
                self.out,
                "<div class=\"code{named}\">{label}<button class=\"copy\" type=\"button\">copy</button><pre><code>"
            );
        }
        *in_code = !*in_code;
        true
    }

    /// A quoted line opens or continues a callout; returns whether the
    /// line was one.
    fn quote(&mut self, line: &str, terms: &[Term]) -> bool {
        let Some(quoted) = line.strip_prefix("> ") else {
            return false;
        };
        if self.callout.is_none() {
            self.flush(terms);
            let (kind, said) = callout_kind(quoted);
            self.callout = Some(kind.to_owned());
            let _written = write!(
                self.out,
                "<aside class=\"callout {}\"><span class=\"mark\">{kind}</span><div class=\"body\">",
                kind.to_lowercase()
            );
            self.paragraph.push_str(said.trim());
        } else {
            if !self.paragraph.is_empty() {
                self.paragraph.push(' ');
            }
            self.paragraph.push_str(quoted.trim());
        }
        true
    }

    /// A list line opens or continues a list, or gathers a reading-guide
    /// card; returns whether the line was one.
    fn list(&mut self, line: &str, terms: &[Term]) -> bool {
        if let Some((kind, item)) = list_item(line) {
            if self.guide {
                self.flush(terms);
                item.trim().clone_into(&mut self.guide_item);
                return true;
            }
            if self.list != Some(kind) {
                self.flush(terms);
                let _written = write!(self.out, "<{kind}>");
                self.list = Some(kind);
            }
            let rendered = self.inline(item.trim(), terms);
            self.remember(&rendered);
            let _written = write!(self.out, "<li>{rendered}</li>");
            return true;
        }
        if !self.guide_item.is_empty() && line.starts_with("  ") {
            self.guide_item.push(' ');
            self.guide_item.push_str(line.trim());
            return true;
        }
        if self.list.is_some() && line.starts_with("  ") {
            let rendered = self.inline(line.trim(), terms);
            self.remember(&rendered);
            let _written = write!(self.out, " {rendered}");
            return true;
        }
        false
    }

    /// A line of prose, joined into the open paragraph.
    fn prose(&mut self, line: &str) {
        if !self.paragraph.is_empty() {
            self.paragraph.push(' ');
        }
        self.paragraph.push_str(line.trim());
    }
}

/// The article, the rail and the index, from the text.
pub(crate) fn render(text: &str, terms: &[Term], meta: &Meta) -> String {
    let mut body = Body {
        numbered: chapters(text),
        ..Body::default()
    };
    let mut lines = text.lines().peekable();
    let mut in_code = false;
    let mut skipping = false;
    while let Some(line) = lines.next() {
        if line.starts_with("<!--") || line.starts_with("-->") || line.starts_with("SPDX-") {
            continue;
        }
        if body.fence(line, terms, &mut in_code) {
            continue;
        }
        if in_code {
            let code = line.strip_prefix("   ").unwrap_or(line);
            let _written = writeln!(body.out, "{}", esc(code));
            body.remember(&esc(code));
            continue;
        }
        if let Some(title) = heading(line) {
            body.flush(terms);
            body.end_guide();
            // The glossary is rendered live at the end, and the book's own
            // contents list is the rail: neither is repeated in the article.
            skipping = title.text.contains("Glossary") || title.text == "Contents";
            if !skipping {
                body.heading(&title);
                if title.level == 2 && title.text == "How to read this" {
                    body.guide = true;
                    body.out.push_str("<div class=\"cards\">");
                }
            }
            continue;
        }
        if skipping || body.quote(line, terms) {
            continue;
        }
        if line.trim().is_empty() {
            body.flush(terms);
            continue;
        }
        if line.trim() == "---" {
            body.flush(terms);
            body.end_guide();
            body.out.push_str("<hr>");
            continue;
        }
        if line.starts_with('|') {
            body.flush(terms);
            body.end_guide();
            let mut rows = vec![line];
            while lines.peek().is_some_and(|next| next.starts_with('|')) {
                if let Some(next) = lines.next() {
                    rows.push(next);
                }
            }
            let rendered = table(&rows, terms, &mut body);
            body.remember(&rendered);
            body.out.push_str(&rendered);
            continue;
        }
        if body.list(line, terms) || line.starts_with("Version of this handbook") {
            // The eyebrow carries the version and the date on the page.
            continue;
        }
        body.prose(line);
    }
    body.flush(terms);
    if in_code {
        body.out.push_str("</code></pre></div>");
    }
    if body.sub_open {
        body.contents.push_str("</ol></li>");
    }
    page(&body, terms, meta)
}

/// One table, header row first, its rule dropped.
pub(crate) fn table(rows: &[&str], terms: &[Term], body: &mut Body) -> String {
    let mut out = String::from("<div class=\"scroll\"><table>");
    for (index, row) in rows.iter().enumerate() {
        if index == 1 && row.contains("---") {
            continue;
        }
        let tag = if index == 0 { "th" } else { "td" };
        out.push_str("<tr>");
        for cell in cells(row) {
            let rendered = body.inline(cell.trim(), terms);
            let _written = write!(out, "<{tag}>{rendered}</{tag}>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</table></div>");
    out
}
