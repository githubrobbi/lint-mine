// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025-2026 Sky, LLC.

//! The Markdown this handbook is written in: headings, paragraphs,
//! tables, fenced code, lists, callouts, links, bold and inline code,
//! and the glossary terms linked into the prose.

use alloc::collections::BTreeMap;

/// One glossary entry: the term as written, and its definition.
pub(crate) struct Term {
    /// The word as the glossary spells it.
    pub(crate) word: String,
    /// The definition, already inline-rendered.
    pub(crate) said: String,
    /// The definition as plain text, for the hover.
    pub(crate) plain: String,
    /// The anchor slug.
    pub(crate) slug: String,
}

/// Escapes text for HTML.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// An anchor slug from a heading or a term.
pub(crate) fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// The glossary: the `**Term** - definition` paragraphs of the last
/// chapter, longest term first so "content object" wins over
/// "content" when both could match.
pub(crate) fn glossary(text: &str) -> Vec<Term> {
    // Found by its title, never by its chapter number: renumbering the
    // book once silently emptied the glossary (2026-09-19).
    let Some(start) = text
        .lines()
        .position(|line| line.starts_with("## ") && line.contains("Glossary"))
        .and_then(|at| {
            text.lines()
                .take(at)
                .map(|line| line.len().saturating_add(1))
                .reduce(usize::saturating_add)
        })
    else {
        return Vec::new();
    };
    let mut terms = Vec::new();
    let mut current: Option<(String, String)> = None;
    let mut header_seen = false;
    for (index, line) in text.get(start..).unwrap_or_default().lines().enumerate() {
        // The chapter ends at the next chapter: what follows is not glossary.
        if index > 0 && line.starts_with("## ") {
            break;
        }
        // The table form: `| Term | Meaning |`, its first row the header.
        if line.trim_start().starts_with('|') {
            if let Some((name, said)) = current.take() {
                terms.push(finish(&name, &said));
            }
            let row = cells(line);
            let rule = row.iter().all(|cell| {
                let cell = cell.trim();
                !cell.is_empty() && cell.chars().all(|ch| matches!(ch, '-' | ':'))
            });
            if rule {
                continue;
            }
            if !header_seen {
                header_seen = true;
                continue;
            }
            if let (Some(word), Some(said)) = (row.first(), row.get(1)) {
                let word = word.trim().trim_matches('*').trim_matches('`').trim();
                if !word.is_empty() {
                    terms.push(finish(word, said.trim()));
                }
            }
            continue;
        }
        if line.trim().is_empty() {
            header_seen = false;
        }
        if let Some((word, tail)) = line
            .strip_prefix("**")
            .and_then(|rest| rest.split_once("**"))
        {
            {
                if let Some((name, said)) = current.take() {
                    terms.push(finish(&name, &said));
                }
                let said = tail.trim_start().trim_start_matches('-').trim().to_owned();
                current = Some((word.to_owned(), said));
                continue;
            }
        }
        if line.trim().is_empty() {
            if let Some((name, said)) = current.take() {
                terms.push(finish(&name, &said));
            }
        } else if let Some((_name, said)) = current.as_mut() {
            said.push(' ');
            said.push_str(line.trim());
        }
    }
    if let Some((name, said)) = current {
        terms.push(finish(&name, &said));
    }
    terms.sort_by_key(|term| core::cmp::Reverse(term.word.len()));
    terms
}

/// One parsed entry, with its rendered and plain forms.
pub(crate) fn finish(word: &str, said: &str) -> Term {
    Term {
        word: word.to_owned(),
        said: inline(said, &[], &mut BTreeMap::new(), &mut BTreeMap::new()),
        plain: said.replace("**", "").replace('`', ""),
        slug: slug(word),
    }
}

/// Inline markup in ONE pass, so a bold run that contains a code
/// span (`**a `b` c**`, which this handbook has) closes where it
/// should. Code spans are verbatim; everything else is escaped, then
/// offered links and glossary terms.
///
/// `seen` says which terms this CHAPTER has already linked, so one
/// link per chapter is offered; `minted` counts every mention in the
/// whole book, so each link gets an anchor of its own and "back" can
/// return to the one that was clicked.
pub(crate) fn inline(
    text: &str,
    terms: &[Term],
    seen: &mut BTreeMap<String, usize>,
    minted: &mut BTreeMap<String, usize>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let mut bold = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '`' {
            out.push_str(&marked(&run, terms, seen, minted));
            run.clear();
            let mut code = String::new();
            for inner in chars.by_ref() {
                if inner == '`' {
                    break;
                }
                code.push(inner);
            }
            let rendered = format!("<code>{}</code>", esc(&code));
            // `PATH` and `WAL` are met in code style first; a span that is
            // exactly a glossary word explains itself like any other.
            let known = terms.iter().find(|term| {
                seen.get(&term.word).copied().unwrap_or(0) == 0
                    && needles(&term.word).iter().any(|needle| {
                        let exact = !needle.chars().any(char::is_lowercase);
                        if exact {
                            *needle == code
                        } else {
                            needle.eq_ignore_ascii_case(&code)
                        }
                    })
            });
            match known {
                Some(term) => out.push_str(&mint(term, &rendered, seen, minted)),
                None => out.push_str(&rendered),
            }
            continue;
        }
        if ch == '*' && chars.peek() == Some(&'*') {
            let _second = chars.next();
            out.push_str(&marked(&run, terms, seen, minted));
            run.clear();
            out.push_str(if bold { "</strong>" } else { "<strong>" });
            bold = !bold;
            continue;
        }
        run.push(ch);
    }
    out.push_str(&marked(&run, terms, seen, minted));
    if bold {
        out.push_str("</strong>");
    }
    out
}

/// Links and glossary terms in a run of plain text.
pub(crate) fn marked(
    text: &str,
    terms: &[Term],
    seen: &mut BTreeMap<String, usize>,
    minted: &mut BTreeMap<String, usize>,
) -> String {
    let mut out = esc(text);
    while let Some(open) = out.find('[') {
        let Some(close) = out.get(open..).and_then(|tail| tail.find("](")) else {
            break;
        };
        let Some(end) = out.get(open..).and_then(|tail| tail.find(')')) else {
            break;
        };
        let label = out
            .get(open.saturating_add(1)..open.saturating_add(close))
            .unwrap_or_default()
            .to_owned();
        let target = out
            .get(open.saturating_add(close).saturating_add(2)..open.saturating_add(end))
            .unwrap_or_default()
            .to_owned();
        let link = format!("<a href=\"{target}\">{label}</a>");
        out.replace_range(open..open.saturating_add(end).saturating_add(1), &link);
    }
    for term in terms {
        if *seen.entry(term.word.clone()).or_insert(0) > 0 {
            continue;
        }
        let Some((at, len)) = needles(&term.word)
            .iter()
            .filter_map(|needle| find_word(&out, needle))
            .min_by_key(|(start, _len)| *start)
        else {
            continue;
        };
        let hit = out
            .get(at..at.saturating_add(len))
            .unwrap_or_default()
            .to_owned();
        let link = mint(term, &hit, seen, minted);
        out.replace_range(at..at.saturating_add(len), &link);
    }
    out
}

/// One link to a glossary entry around `shown`, with an anchor of its
/// own so the entry's "back" can return to exactly this mention.
fn mint(
    term: &Term,
    shown: &str,
    seen: &mut BTreeMap<String, usize>,
    minted: &mut BTreeMap<String, usize>,
) -> String {
    seen.insert(term.word.clone(), 1);
    let nth = minted
        .entry(term.word.clone())
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
    format!(
        "<a class=\"term\" id=\"t-{}-{nth}\" data-key=\"{}\" href=\"#g-{}\" data-def=\"{}\">{shown}</a>",
        term.slug,
        term.slug,
        term.slug,
        esc(&term.plain)
    )
}

/// The spellings a glossary head offers the prose: `Sidecar / blob`
/// is two words for one thing, and `Unit (passage)` is a word and its
/// gloss. The drive of 2026-09-19 found three entries whose "back"
/// pointed nowhere because only the literal head was ever searched.
pub(crate) fn needles(word: &str) -> Vec<String> {
    let mut out: Vec<String> = word
        .split(" / ")
        .map(|piece| piece.split(" (").next().unwrap_or(piece).trim().to_owned())
        .filter(|piece| !piece.is_empty())
        .collect();
    // The prose does not always use the glossary's head word: it says
    // "semantic" where the entry is "Semantic search", "embedding" where
    // it is "Vector". These are the spellings the chapters really use.
    for (head, also) in ALIASES {
        if *head == word {
            out.extend(also.iter().map(|alias| (*alias).to_owned()));
        }
    }
    // "entity" is also "entities".
    let plurals: Vec<String> = out
        .iter()
        .filter_map(|needle| needle.strip_suffix('y').map(|stem| format!("{stem}ies")))
        .collect();
    out.extend(plurals);
    out
}

/// The other spellings of a glossary head that the chapters use.
const ALIASES: &[(&str, &[&str])] = &[
    ("Semantic search", &["semantic"]),
    ("Keyword search", &["keyword", "lexical"]),
    ("Sparse terms", &["sparse"]),
    ("Dense vectors", &["dense"]),
    ("Vector", &["embedding"]),
    ("ONNX Runtime", &["ONNX"]),
    ("Reranker", &["re-ranker", "rerank", "reranking"]),
    ("Fingerprint", &["hash", "checksum"]),
    ("Extraction", &["extract", "extractor"]),
    ("Compaction", &["compact"]),
    ("Theme", &["communities", "community"]),
    ("Scheduler", &["launchd", "Task Scheduler", "cron"]),
];

/// The first whole-word, case-insensitive occurrence of `word` that is
/// not already inside a tag or an attribute, as `(start, length)`. A
/// plural counts: the prose says "file instances" where the glossary
/// says "File instance".
pub(crate) fn find_word(haystack: &str, word: &str) -> Option<(usize, usize)> {
    // An acronym is matched as written: PATH is a glossary word, the
    // "path" of a file is not.
    let exact = word.len() >= 2 && !word.chars().any(char::is_lowercase);
    let lower = if exact {
        haystack.to_owned()
    } else {
        haystack.to_lowercase()
    };
    let needle = if exact {
        word.to_owned()
    } else {
        word.to_lowercase()
    };
    let mut from = 0;
    while let Some(at) = lower.get(from..)?.find(&needle) {
        let start = from.saturating_add(at);
        let bare_end = start.saturating_add(needle.len());
        let tail = lower.get(bare_end..)?;
        let plural = ["es", "s"]
            .iter()
            .find(|suffix| {
                tail.strip_prefix(**suffix)
                    .is_some_and(|rest| rest.chars().next().is_none_or(|ch| !ch.is_alphanumeric()))
            })
            .map_or(0, |suffix| suffix.len());
        let end = bare_end.saturating_add(plural);
        let before = lower.get(..start)?.chars().next_back();
        let after = lower.get(end..)?.chars().next();
        let whole = before.is_none_or(|ch| !ch.is_alphanumeric())
            && after.is_none_or(|ch| !ch.is_alphanumeric());
        let head = lower.get(..start)?;
        let inside_tag = head.rfind('<') > head.rfind('>');
        // Never a link inside a link: "content object" is already one.
        let inside_link = head.rfind("<a ") > head.rfind("</a>");
        if whole
            && !inside_tag
            && !inside_link
            && haystack.is_char_boundary(start)
            && haystack.is_char_boundary(end)
        {
            return Some((start, end.saturating_sub(start)));
        }
        from = bare_end;
    }
    None
}

/// One table row split on UNESCAPED pipes only: this handbook writes
/// `\|` inside a code span for a command's alternatives, and splitting
/// on those tore the cell in two and left a dangling backtick.
pub(crate) fn cells(row: &str) -> Vec<String> {
    let trimmed = row.trim().trim_start_matches('|').trim_end_matches('|');
    let mut out = vec![String::new()];
    let mut escaped = false;
    for ch in trimmed.chars() {
        if escaped {
            if let Some(last) = out.last_mut() {
                if ch != '|' {
                    last.push('\\');
                }
                last.push(ch);
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '|' => out.push(String::new()),
            other => {
                if let Some(last) = out.last_mut() {
                    last.push(other);
                }
            }
        }
    }
    out
}

/// A callout's kind from its first words: `**Note:** ...`, `Tip:`,
/// `Warning:`; anything else is a note.
pub(crate) fn callout_kind(text: &str) -> (&'static str, &str) {
    let bare = text.trim().trim_start_matches("**");
    for kind in ["Note", "Tip", "Warning"] {
        if let Some(after) = bare.strip_prefix(kind) {
            let said = after
                .trim_start_matches(':')
                .trim_start_matches("**")
                .trim_start_matches(':');
            return (kind, said);
        }
    }
    ("Note", text)
}

/// A heading line: its level, its text and its anchor.
pub(crate) struct Heading<'a> {
    /// One to six.
    pub(crate) level: usize,
    /// The words after the hashes.
    pub(crate) text: &'a str,
    /// The anchor slug.
    pub(crate) id: String,
}

/// The heading a line opens, if it opens one.
pub(crate) fn heading(line: &str) -> Option<Heading<'_>> {
    let level = line
        .find(|ch| ch != '#')
        .filter(|at| *at > 0 && *at <= 6 && line.get(*at..at.saturating_add(1)) == Some(" "))?;
    let text = line.get(level.saturating_add(1)..)?.trim();
    Some(Heading {
        level,
        text,
        id: slug(text),
    })
}

/// A list line: which kind of list it opens, and the item's text.
pub(crate) fn list_item(line: &str) -> Option<(&'static str, &str)> {
    let body = line.trim_start();
    if let Some(item) = body.strip_prefix("- ") {
        return Some(("ul", item));
    }
    body.split_once(". ")
        .filter(|(head, _rest)| !head.is_empty() && head.chars().all(|ch| ch.is_ascii_digit()))
        .map(|(_head, item)| ("ol", item))
}
