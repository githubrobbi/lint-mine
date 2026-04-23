// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025-2026 Sky, LLC.

//! The page around the article: the shared stylesheet and theme
//! switch the binaries carry, the handbook's own layout, the search
//! index and the script that makes it live.

use core::fmt::Write as _;

use crate::markdown::{Term, esc};
use crate::{Body, Section, today};

/// The search index as JSON, written by hand so the tool carries no
/// serializer for four fields.
pub(crate) fn index_json(sections: &[Section]) -> String {
    fn quote(text: &str) -> String {
        let mut out = String::with_capacity(text.len().saturating_add(2));
        out.push('"');
        for ch in text.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' | '\r' | '\t' => out.push(' '),
                '<' => out.push_str("\\u003c"),
                other if (other as u32) < 0x20 => {}
                other => out.push(other),
            }
        }
        out.push('"');
        out
    }
    let mut out = String::from("[");
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _written = write!(
            out,
            "{{\"id\":{},\"t\":{},\"c\":{},\"x\":{}}}",
            quote(&section.id),
            quote(&section.title),
            quote(&section.chapter),
            quote(&section.text)
        );
    }
    out.push(']');
    out
}

/// The whole page around the article.
pub(crate) fn page(body: &Body, terms: &[Term], meta: &Meta) -> String {
    // Shown as a dictionary is: by the alphabet. (The matching order,
    // longest word first, is the renderer's business and not the reader's.)
    let mut shown: Vec<&Term> = terms.iter().collect();
    shown.sort_by_key(|term| term.word.to_lowercase());
    let entries = shown.into_iter().fold(String::new(), |mut out, term| {
        // A way back only where the prose links the word at all: an
        // entry nothing points at has nowhere to return to.
        let back = if body.minted.contains_key(&term.word) {
            format!(
                "<a class=\"back\" data-back=\"{}\" href=\"#t-{}-1\">back</a>",
                term.slug, term.slug
            )
        } else {
            String::new()
        };
        let _written = write!(
            out,
            "<dt id=\"g-{}\">{}{back}</dt><dd>{}</dd>",
            term.slug,
            esc(&term.word),
            term.said
        );
        out
    });
    let glossary = if terms.is_empty() {
        String::new()
    } else {
        format!(
            "<h2 id=\"glossary\">The words in this book<a class=\"anchor\" href=\"#glossary\" \
aria-label=\"Link to this section\">#</a></h2>\
<p>Each dotted word in the chapters shows its meaning when you rest on it and jumps here when \
you click it. Every entry has a <strong>back</strong> that returns you to the line you were \
reading.</p><dl>{entries}</dl>"
        )
    };
    let stamp = today();
    let title = esc(&meta.title);
    let source = esc(&meta.source);
    let large = if meta.large { LARGE } else { "" };
    // The glossary is searchable too: "what is a vector" lands on its entry.
    let words = terms.iter().map(|term| Section {
        id: format!("g-{}", term.slug),
        title: term.word.clone(),
        chapter: String::from("The words in this book"),
        text: term.plain.clone(),
    });
    let mut sections: Vec<Section> = words.collect();
    sections.extend(body.sections.iter().map(|section| Section {
        id: section.id.clone(),
        title: section.title.clone(),
        chapter: section.chapter.clone(),
        text: section.text.clone(),
    }));
    let index = index_json(&sections);
    // REUSE-IgnoreStart - the lines below are the GENERATED page's own
    // header, a template rather than this file's declaration; this file
    // declares itself at the top like every other.
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
\n<!-- SPDX-FileCopyrightText: 2025-2026 Sky, LLC. -->\
\n<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->\
\n<!-- Generated from {source} by md2html; edit the Markdown, not this page. -->\n\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex,nofollow\">\
<title>{title}</title><style>{BASE_STYLE}{STYLE}{large}</style>\
<script>{THEME}</script></head><body>\
<a class=\"skip\" href=\"#main\">Skip to content</a><div id=\"bar\"></div>\
<div class=\"bar\"><div class=\"bar-in\">\
<a class=\"brand\" href=\"#top\">{GLYPH}md2html</a><span class=\"kicker\">{source}</span>\
<span class=\"crumb\" id=\"crumb\"></span><span class=\"grow\"></span>\
<button id=\"open-find\" class=\"btn\" type=\"button\"><svg viewBox=\"0 0 24 24\">\
<circle cx=\"11\" cy=\"11\" r=\"7\"/><path d=\"m20 20-3.5-3.5\"/></svg><span class=\"lbl\">Search</span><kbd>&#8984;K</kbd></button>\
<button id=\"toggle\" class=\"btn icon\" type=\"button\" aria-label=\"Contents\"><svg viewBox=\"0 0 24 24\">\
<path d=\"M4 7h16M4 12h16M4 17h10\"/></svg></button>{SIZE_BUTTONS}{THEME_BUTTON}</div></div>\
<div class=\"wrap\" id=\"top\">\
<nav class=\"toc\" aria-label=\"Contents\"><ol>{contents}\
<li><a href=\"#glossary\">The words in this book</a></li></ol></nav>\
<article id=\"main\"><header class=\"hero\"><div class=\"eyebrow\">{source} &middot; {stamp}</div>\
{article}{glossary}\
<footer class=\"fine\">Rendered from <code>{source}</code> by <code>md2html</code>. \
The Markdown file is the source; render again after editing it.</footer></article></div>\
<dialog id=\"find\"><div class=\"find-in\"><input id=\"q\" type=\"search\" autocomplete=\"off\" \
placeholder=\"Search this page\" aria-label=\"Search this page\">\
<ol id=\"hits\" role=\"listbox\"></ol><div class=\"find-foot\"><kbd>&uarr;</kbd><kbd>&darr;</kbd> move \
<kbd>&#9166;</kbd> open <kbd>esc</kbd> close</div></div></dialog>\
<script id=\"idx\" type=\"application/json\">{index}</script>\
<script>{BEHAVE}</script><script>{SCRIPT}</script></body></html>",
        // REUSE-IgnoreEnd
        contents = body.contents,
        article = body.out,
    )
}

/// The page layer: tokens, then chrome (carried over from docenta's page
/// crate, D85).
pub(crate) const BASE_STYLE: &str = concat!(
    include_str!("page/tokens.css"),
    include_str!("page/style.css")
);
/// The theme switch.
pub(crate) const THEME: &str = include_str!("page/theme.js");
/// The glossary's behaviour and the find dialog's keys.
const BEHAVE: &str = include_str!("page/behave.js");
/// The brand mark.
pub(crate) const GLYPH: &str = include_str!("page/glyph.svg");

/// The larger type for pages read at half a laptop screen (`--large`).
const LARGE: &str = include_str!("page/large.css");

/// What the page says about itself: its title, the file it came from,
/// and whether it uses the larger type.
pub(crate) struct Meta {
    /// The first level-one heading, or the file name.
    pub(crate) title: String,
    /// The Markdown file's name.
    pub(crate) source: String,
    /// Append `page/large.css`.
    pub(crate) large: bool,
}
/// The switch button.
pub(crate) const THEME_BUTTON: &str = "<button id=\"theme\" class=\"btn icon\" type=\"button\" \
title=\"Theme\">\
<svg class=\"sun\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"4\"/>\
<path d=\"M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4\"/></svg>\
<svg class=\"moon\" viewBox=\"0 0 24 24\"><path d=\"M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z\"/></svg>\
<svg class=\"auto\" viewBox=\"0 0 24 24\"><rect x=\"3\" y=\"4\" width=\"18\" height=\"12\" rx=\"2\"/>\
<path d=\"M8 20h8M12 16v4\"/></svg></button>";
/// The text-size group: smaller, the current percent (click: back to
/// 100), larger. The script fills the labels and disables the ends.
pub(crate) const SIZE_BUTTONS: &str = "<span class=\"size\" role=\"group\" aria-label=\"Text size\">\
<button id=\"size-down\" class=\"btn icon\" type=\"button\" title=\"Smaller text\">\
<svg viewBox=\"0 0 24 24\"><path d=\"M5 12h14\"/></svg></button>\
<button id=\"size\" class=\"btn\" type=\"button\" title=\"Text size\">100%</button>\
<button id=\"size-up\" class=\"btn icon\" type=\"button\" title=\"Larger text\">\
<svg viewBox=\"0 0 24 24\"><path d=\"M12 5v14M5 12h14\"/></svg></button></span>";
/// The handbook's own layout on top of the shared stylesheet: the
/// rail, the hero, the progress line, the search dialog.
pub(crate) const STYLE: &str = "\
.wrap{max-width:1320px;margin:0 auto;padding:0 24px;display:grid;grid-template-columns:252px minmax(0,1fr);gap:56px;align-items:start}\
nav.toc{position:sticky;top:54px;max-height:calc(100vh - 54px);overflow:auto;padding:28px 10px 40px 0;font-size:14px;scrollbar-width:thin}\
nav.toc ol{list-style:none;margin:0;padding:0}\
nav.toc>ol>li{margin:0 0 2px}\
nav.toc a{display:block;color:var(--text-2);padding:6px 10px;border-radius:8px;border-left:2px solid transparent;line-height:1.35}\
nav.toc a:hover{color:var(--text);background:var(--surface);text-decoration:none}\
nav.toc>ol>li>a.on{color:var(--text);background:var(--surface);border-left-color:var(--accent);font-weight:600}\
nav.toc ol.sub{display:none;margin:2px 0 8px 14px;border-left:1px solid var(--line)}\
nav.toc li.open>ol.sub{display:block}\
nav.toc ol.sub a{font-size:13px;padding:4px 10px;border-radius:0 8px 8px 0}\
nav.toc ol.sub a.on{color:var(--accent);font-weight:500}\
article{padding:40px 0 110px;min-width:0;max-width:none;container-type:inline-size}\
article>:not(.scroll){max-width:820px}\
article .scroll{left:0;transform:none;max-width:100cqw}\
.hero{padding:0 0 4px;margin:0}\
.eyebrow{font:500 11.5px var(--mono);letter-spacing:.16em;text-transform:uppercase;color:var(--label);margin-bottom:16px;display:flex;gap:12px;align-items:center}\
.eyebrow::before{content:'';width:28px;height:1px;background:var(--label);opacity:.6;flex:none}\
.hero h1{font-size:clamp(34px,5vw,54px);margin-bottom:18px}\
.hero .lead{font-size:19px;line-height:1.55}\
#bar{position:fixed;top:0;left:0;height:2px;width:0;background:linear-gradient(90deg,var(--glow),var(--accent-2));z-index:60}\
dialog#find{border:1px solid var(--line-2);border-radius:14px;background:var(--surface);color:var(--text);padding:0;width:min(640px,92vw);box-shadow:var(--shadow);margin:10vh auto 0}\
dialog#find::backdrop{background:rgba(15,13,11,.45);backdrop-filter:blur(3px)}\
.find-in{display:flex;flex-direction:column}\
#q{font:17px var(--sans);padding:16px 18px;border:0;border-bottom:1px solid var(--line);background:transparent;color:var(--text);outline:none;width:100%}\
#hits{list-style:none;margin:0;padding:6px;max-height:52vh;overflow:auto}\
#hits li{padding:10px 12px;border-radius:9px;cursor:pointer;display:grid;gap:3px}\
#hits li.on,#hits li:hover{background:var(--surface-2)}\
#hits .h{font-weight:600;color:var(--text);font-size:15px}\
#hits .c{font:500 10.5px var(--mono);letter-spacing:.12em;text-transform:uppercase;color:var(--label)}\
#hits .s{font-size:13.5px;color:var(--text-2);line-height:1.45}\
#hits .none{color:var(--text-2);cursor:default}\
.find-foot{display:flex;gap:10px;align-items:center;padding:8px 14px;border-top:1px solid var(--line);font-size:12px;color:var(--text-2)}\
.find-foot kbd{font:11px var(--mono);border:1px solid var(--line-2);border-radius:5px;padding:1px 5px;color:var(--text-2)}\
mark{background:color-mix(in srgb,var(--glow) 45%,transparent);color:inherit;border-radius:3px;padding:0 2px}\
#toggle{display:none}\
@media (max-width:980px){.wrap{grid-template-columns:minmax(0,1fr);gap:0}#toggle{display:inline-flex}\
nav.toc{display:none;position:fixed;inset:54px 0 0 0;background:var(--bg);z-index:35;padding:20px 24px;max-height:none;border-top:1px solid var(--line)}\
nav.toc.open{display:block}article{padding-top:28px;max-width:none}}\
@media (max-width:640px){.kicker,#open-find kbd,#open-find .lbl{display:none}.eyebrow{font-size:10.5px;letter-spacing:.12em}.hero h1{font-size:32px}}";

/// The page's script: the search, the rail that follows the reader,
/// the crumb in the bar, the copy buttons, the glossary's way back,
/// the progress line. Everything reads without it.
pub(crate) const SCRIPT: &str = "\
(function(){\
var $=function(s,r){return (r||document).querySelector(s);},$$=function(s,r){return [].slice.call((r||document).querySelectorAll(s));};\
var bar=$('#bar');addEventListener('scroll',function(){var h=document.documentElement;var p=h.scrollTop/((h.scrollHeight-h.clientHeight)||1);bar.style.width=(p*100)+'%';},{passive:true});\
var land=window.ivLand;\
document.addEventListener('click',function(e){var t=e.target.closest('nav.toc a');if(t){$('nav.toc').classList.remove('open');}});\
var tg=$('#toggle');if(tg)tg.addEventListener('click',function(){$('nav.toc').classList.toggle('open');});\
var links={},crumb=$('#crumb'),heads=$$('article h2[id],article h3[id]');\
$$('nav.toc a[href^=\"#\"]').forEach(function(a){links[a.getAttribute('href').slice(1)]=a;});\
var current=null;\
function mark(h){if(!h||h===current)return;current=h;\
$$('nav.toc a.on').forEach(function(a){a.classList.remove('on');});$$('nav.toc li.open').forEach(function(li){li.classList.remove('open');});\
var a=links[h.id];if(!a)return;a.classList.add('on');var li=a.parentNode;var top=li.closest('nav.toc>ol>li');if(top){top.classList.add('open');var ta=top.querySelector(':scope>a');if(ta)ta.classList.add('on');}\
var ch=top?top.querySelector(':scope>a').textContent:'';var sec=h.tagName==='H3'?h.firstChild.textContent||'':'';\
crumb.textContent=sec?ch+'  \\u203a  '+sec:ch;a.scrollIntoView({block:'nearest'});}\
function spy(){var y=scrollY+120,best=null;for(var i=0;i<heads.length;i++){if(heads[i].offsetTop<=y)best=heads[i];else break;}mark(best||heads[0]);}\
addEventListener('scroll',spy,{passive:true});addEventListener('load',spy);spy();\
var dlg=$('#find'),q=$('#q'),hits=$('#hits'),idx=[];try{idx=JSON.parse($('#idx').textContent);}catch(e){}\
var sel=0,found=[];\
function esc(s){return s.replace(/[&<>]/g,function(c){return {'&':'&amp;','<':'&lt;','>':'&gt;'}[c];});}\
function snippet(x,words){var lx=x.toLowerCase(),at=-1;for(var i=0;i<words.length;i++){var p=lx.indexOf(words[i]);if(p>=0&&(at<0||p<at))at=p;}\
if(at<0)at=0;var s=Math.max(0,at-60),e=Math.min(x.length,at+120),out=esc(x.slice(s,e));\
words.forEach(function(w){if(!w)return;out=out.replace(new RegExp('('+w.replace(/[.*+?^${}()|[\\]\\\\]/g,'\\\\$&')+')','ig'),'<mark>$1</mark>');});\
return (s>0?'\\u2026':'')+out+(e<x.length?'\\u2026':'');}\
function search(){var words=q.value.toLowerCase().split(/\\s+/).filter(Boolean);found=[];hits.innerHTML='';sel=0;if(!words.length)return;\
var phrase=words.join(' ');\
idx.forEach(function(s){var t=s.t.toLowerCase(),x=s.x.toLowerCase(),score=0;for(var i=0;i<words.length;i++){var w=words[i],inT=t.indexOf(w)>=0,n=Math.min(x.split(w).length-1,4);if(!inT&&!n){score=0;break;}score+=(inT?6:0)+n;}\
if(score){if(t===phrase)score+=20;if(words.length>1&&(t.indexOf(phrase)>=0||x.indexOf(phrase)>=0))score+=8;found.push({s:s,score:score});}});\
found.sort(function(a,b){return b.score-a.score;});found=found.slice(0,14);\
if(!found.length){hits.innerHTML='<li class=\"none\">Nothing in the book matches that.</li>';return;}\
found.forEach(function(f,i){var li=document.createElement('li');li.setAttribute('role','option');if(i===0)li.className='on';\
li.innerHTML='<span class=\"c\">'+esc(f.s.c)+'</span><span class=\"h\">'+esc(f.s.t)+'</span><span class=\"s\">'+snippet(f.s.x,words)+'</span>';\
li.addEventListener('click',function(){go(i);});hits.appendChild(li);});}\
function go(i){var f=found[i];if(!f)return;dlg.close();location.hash='#'+f.s.id;land(f.s.id);}\
function open(){if(!dlg.open){dlg.showModal();q.select();}}\
$('#open-find').addEventListener('click',open);\
q.addEventListener('input',search);\
q.addEventListener('keydown',function(e){var n=hits.children.length;if(e.key==='ArrowDown'&&n){e.preventDefault();sel=(sel+1)%n;}else if(e.key==='ArrowUp'&&n){e.preventDefault();sel=(sel-1+n)%n;}else if(e.key==='Enter'){e.preventDefault();go(sel);return;}else return;\
$$('#hits li').forEach(function(li,i){li.classList.toggle('on',i===sel);});var on=hits.children[sel];if(on)on.scrollIntoView({block:'nearest'});});\
dlg.addEventListener('click',function(e){if(e.target===dlg)dlg.close();});\
document.addEventListener('keydown',function(e){if((e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==='k'){e.preventDefault();open();}\
else if(e.key==='/'&&!dlg.open&&!/input|textarea/i.test(document.activeElement.tagName)){e.preventDefault();open();}});\
if(location.hash)land(location.hash.slice(1));\
})();";
