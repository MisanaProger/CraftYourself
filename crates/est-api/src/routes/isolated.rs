//! Untrusted documents — bundled artifacts and HTML pages written by a
//! model — are served with a CSP `sandbox` directive: they get an opaque
//! origin (no access to the platform's API, cookies or storage, even when
//! opened directly) and may not load anything from the network. The
//! interface additionally shows them in `<iframe sandbox="allow-scripts">`.

use axum::http::header;
use axum::response::{IntoResponse, Response};

const CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; \
     style-src 'unsafe-inline'; img-src data:; font-src 'none'; connect-src 'none'; \
     form-action 'none'; base-uri 'none'; frame-ancestors 'self'";

/// Pages written by a model may also use `eval`/`new Function` (formula
/// evaluators, small interpreters): they already run arbitrary inline
/// script, so this adds no capability; isolation is the sandbox and the
/// network ban, which stay the same.
const MODEL_CSP: &str = "sandbox allow-scripts; default-src 'none'; \
     script-src 'unsafe-inline' 'unsafe-eval'; style-src 'unsafe-inline'; img-src data:; \
     font-src 'none'; connect-src 'none'; form-action 'none'; base-uri 'none'; \
     frame-ancestors 'self'";

fn with_csp(bytes: impl IntoResponse, mime: &'static str, csp: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CONTENT_SECURITY_POLICY, csp),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        bytes,
    )
        .into_response()
}

pub fn document(bytes: impl IntoResponse, mime: &'static str) -> Response {
    with_csp(bytes, mime, CSP)
}

/// Runs before the page's own scripts. A model writes pages as if for an
/// ordinary tab; in the sandbox some web APIs are missing or blocked, and
/// this layer makes them behave instead of breaking the page:
///
/// * `localStorage`/`sessionStorage` throw in an opaque origin → in-memory
///   storage that lives while the page is open;
/// * `alert`/`confirm`/`prompt` are ignored without `allow-modals` (which
///   would let a page block the whole tab) → a message inside the page;
///   `confirm` answers yes, `prompt` its default;
/// * form submission is blocked before the `submit` event → submit buttons,
///   Enter and `requestSubmit()` dispatch `submit` to the page's handlers;
///   nothing is sent anywhere (`form-action 'none'`);
/// * `?theme=light|dark` from the platform → `data-theme` and
///   `color-scheme` on `<html>`;
/// * the page's height → `est:resize`, so the frame fits the content. A
///   page sized by the viewport (`100vh`) would grow with every resize; when
///   the overflow stays the same while the frame grows, reporting stops.
const SHIM: &str = r#"<script>(function(){
var root=document.documentElement;
var theme=/[?&]theme=(light|dark)\b/.exec(location.search);
if(theme){root.setAttribute("data-theme",theme[1]);root.style.colorScheme=theme[1];}
function memory(){var d=Object.create(null);return{
getItem:function(k){k=String(k);return k in d?d[k]:null;},setItem:function(k,v){d[String(k)]=String(v);},
removeItem:function(k){delete d[String(k)];},clear:function(){d=Object.create(null);},
key:function(i){var k=Object.keys(d);return i<k.length?k[i]:null;},get length(){return Object.keys(d).length;}};}
["localStorage","sessionStorage"].forEach(function(n){try{window[n].length;}catch(e){
try{Object.defineProperty(window,n,{value:memory(),configurable:true});}catch(_){}}});
var box=null;
function note(text){if(!document.body){addEventListener("DOMContentLoaded",function(){note(text);});return;}
if(!box){box=document.createElement("div");box.setAttribute("role","status");
box.style.cssText="position:fixed;left:50%;bottom:12px;transform:translateX(-50%);max-width:90%;"+
"z-index:2147483647;padding:8px 14px;border-radius:8px;background:#222;color:#fff;"+
"font:14px/1.4 system-ui,sans-serif;box-shadow:0 2px 10px rgba(0,0,0,.3);white-space:pre-wrap";}
box.textContent=String(text);document.body.appendChild(box);clearTimeout(box.t);
box.t=setTimeout(function(){box.remove();},4000);}
window.alert=function(m){note(m===undefined?"":m);};
window.confirm=function(m){note(m===undefined?"":m);return true;};
window.prompt=function(m,v){note(m===undefined?"":m);return v===undefined?"":String(v);};
function submit(form,by){if(!form.noValidate&&!(by&&by.formNoValidate)&&!form.reportValidity())return;
var ev;try{ev=new SubmitEvent("submit",{bubbles:true,cancelable:true,submitter:by||null});}
catch(e){ev=new Event("submit",{bubbles:true,cancelable:true});}form.dispatchEvent(ev);}
HTMLFormElement.prototype.requestSubmit=function(by){submit(this,by);};
document.addEventListener("click",function(e){if(e.defaultPrevented)return;
var b=e.target&&e.target.closest&&e.target.closest("button,input");if(!b||!b.form)return;
var t=(b.getAttribute("type")||(b.tagName==="BUTTON"?"submit":"text")).toLowerCase();
if(t!=="submit"&&t!=="image")return;e.preventDefault();submit(b.form,b);});
document.addEventListener("keydown",function(e){if(e.key!=="Enter"||e.defaultPrevented)return;
var i=e.target;if(!i||i.tagName!=="INPUT"||!i.form)return;
if(i.form.querySelector("button:not([type]),button[type=submit],input[type=submit],input[type=image]"))return;
e.preventDefault();submit(i.form,null);});
var last=0,gap=null,stopped=false;
function report(){if(stopped||!document.body)return;
var h=Math.ceil(Math.max(document.body.scrollHeight,root.offsetHeight));
if(Math.abs(h-last)<=2)return;var g=h-innerHeight;
if(h>last&&last>0&&gap!==null&&Math.abs(g-gap)<=2&&g>0){stopped=true;return;}
gap=g;last=h;parent.postMessage({type:"est:resize",version:1,height:h},"*");}
addEventListener("DOMContentLoaded",function(){report();
if(window.ResizeObserver){var o=new ResizeObserver(report);o.observe(root);o.observe(document.body);}});
addEventListener("load",report);
})();</script>"#;

/// Inserts the compatibility layer before any of the page's own content:
/// after `<head …>`, else after `<html …>`, else after the doctype (never
/// before it, which would switch the page to quirks mode).
fn with_shim(mut html: String) -> String {
    let lower = html.to_ascii_lowercase();
    let after_tag = |tag: &str| {
        let mut from = 0;
        while let Some(i) = lower[from..].find(tag).map(|i| i + from) {
            let next = lower.as_bytes().get(i + tag.len()).copied();
            if matches!(next, Some(b'>' | b' ' | b'\t' | b'\n' | b'\r')) {
                return lower[i..].find('>').map(|j| i + j + 1);
            }
            from = i + tag.len();
        }
        None
    };
    let at = after_tag("<head")
        .or_else(|| after_tag("<html"))
        .or_else(|| after_tag("<!doctype"))
        .unwrap_or(0);
    html.insert_str(at, SHIM);
    html
}

/// An HTML page written by a model, with the compatibility layer.
pub fn model_page(html: String) -> Response {
    with_csp(with_shim(html), "text/html; charset=utf-8", MODEL_CSP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_runs_before_the_page() {
        let html = with_shim("<!DOCTYPE html><HTML lang=en><head><title>x</title>".into());
        assert!(html.starts_with("<!DOCTYPE html><HTML lang=en><head><script>"));
        let html = with_shim("<!doctype html><header>x</header><p>".into());
        assert!(html.starts_with("<!doctype html><script>"), "{html}");
        assert!(with_shim("<p>x".into()).starts_with("<script>"));
        for csp in [CSP, MODEL_CSP] {
            assert!(csp.starts_with("sandbox allow-scripts;"));
            assert!(csp.contains("connect-src 'none'") && !csp.contains("allow-same-origin"));
        }
        let r = model_page("<p>x".into());
        assert_eq!(r.headers()[header::CONTENT_SECURITY_POLICY], MODEL_CSP);
    }
}
