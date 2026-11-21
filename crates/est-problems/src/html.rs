//! Problem statements from the judges' HTML (stage 3: loading statements
//! with the learner's permission).
//!
//! Codeforces serves the statement as an HTML page, LeetCode's API returns
//! it as an HTML fragment. Both become the Markdown text the learner would
//! otherwise paste, plus the samples. The conversion is deliberately small:
//! text, paragraphs, lists, `<pre>`, inline code, sub/superscripts and
//! entities; scripts, styles and images are dropped. The result goes through
//! [`crate::normalize`] like a pasted statement, so its limits apply.

use crate::Sample;

/// Decodes the HTML entities judges use.
pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[..rest.len().min(12)].find(';');
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            let c = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                "le" => Some('≤'),
                "ge" => Some('≥'),
                "ne" => Some('≠'),
                "times" => Some('×'),
                "minus" => Some('−'),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "middot" => Some('·'),
                "rarr" => Some('→'),
                "larr" => Some('←'),
                "infin" => Some('∞'),
                _ => {
                    let num = name.strip_prefix('#')?;
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => num.parse().ok()?,
                    };
                    char::from_u32(code).filter(|c| *c != '\0')
                }
            }?;
            Some((c, e + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `<div class="a b">` → (`div`, closing, attributes).
fn parse_tag(tag: &str) -> (String, bool, &str) {
    let t = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let (closing, t) = match t.strip_prefix('/') {
        Some(r) => (true, r.trim_start()),
        None => (false, t),
    };
    let end = t
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(t.len());
    (t[..end].to_ascii_lowercase(), closing, &t[end..])
}

fn has_class(attrs: &str, class: &str) -> bool {
    let Some(i) = attrs.find("class=") else {
        return false;
    };
    let v = &attrs[i + 6..];
    let quote = v.chars().next().unwrap_or(' ');
    let v = if quote == '"' || quote == '\'' {
        let v = &v[1..];
        &v[..v.find(quote).unwrap_or(v.len())]
    } else {
        &v[..v.find(char::is_whitespace).unwrap_or(v.len())]
    };
    v.split_whitespace().any(|c| c == class)
}

/// Converts an HTML fragment to Markdown text.
pub fn to_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut pre = 0usize;
    let mut skip: Option<String> = None;
    let mut list_depth = 0usize;
    let mut rest = html;
    let newline = |out: &mut String, n: usize| {
        let trailing = out.chars().rev().take_while(|c| *c == '\n').count();
        if !out.is_empty() {
            for _ in trailing..n {
                out.push('\n');
            }
        }
    };
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if skip.is_none() {
                push_text(&mut out, rest, pre > 0);
            }
            break;
        };
        if lt > 0 && skip.is_none() {
            push_text(&mut out, &rest[..lt], pre > 0);
        }
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map(|e| &r[e + 3..]).unwrap_or("");
            continue;
        }
        let Some(gt) = rest.find('>') else {
            break;
        };
        let (name, closing, attrs) = parse_tag(&rest[..=gt]);
        rest = &rest[gt + 1..];
        if let Some(s) = &skip {
            if closing && *s == name {
                skip = None;
            }
            continue;
        }
        match (name.as_str(), closing) {
            ("script" | "style" | "noscript" | "svg", false) => skip = Some(name),
            ("br", _) => out.push('\n'),
            ("p" | "div" | "section" | "table" | "tr", _) => {
                if pre > 0 {
                    if closing {
                        newline(&mut out, 1)
                    }
                } else {
                    newline(&mut out, 2)
                }
                if !closing && has_class(attrs, "section-title") {
                    out.push_str("### ");
                }
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                newline(&mut out, 2);
                out.push_str("### ");
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => newline(&mut out, 2),
            ("ul" | "ol", false) => {
                list_depth += 1;
                newline(&mut out, 1);
            }
            ("ul" | "ol", true) => {
                list_depth = list_depth.saturating_sub(1);
                newline(&mut out, 2);
            }
            ("li", false) => {
                newline(&mut out, 1);
                out.push_str(&"  ".repeat(list_depth.saturating_sub(1)));
                out.push_str("- ");
            }
            ("pre", false) => {
                newline(&mut out, 2);
                out.push_str("```\n");
                pre += 1;
                // A newline right after `<pre>` is not content.
                rest = rest
                    .strip_prefix("\r\n")
                    .or_else(|| rest.strip_prefix('\n'))
                    .unwrap_or(rest);
            }
            ("pre", true) => {
                pre = pre.saturating_sub(1);
                newline(&mut out, 1);
                out.push_str("```");
                newline(&mut out, 2);
            }
            ("code", _) if pre == 0 => out.push('`'),
            ("strong" | "b", _) if pre == 0 => out.push_str("**"),
            ("em" | "i", _) if pre == 0 => out.push('*'),
            ("sup", false) => out.push('^'),
            ("sub", false) => out.push('_'),
            ("td" | "th", true) => out.push(' '),
            _ => {}
        }
    }
    tidy_markdown(&out)
}

fn push_text(out: &mut String, text: &str, pre: bool) {
    let text = decode_entities(text);
    if pre {
        out.push_str(&text);
        return;
    }
    for (i, word) in text.split_whitespace().enumerate() {
        let starts_with_space = text.starts_with(char::is_whitespace);
        if (i > 0 || starts_with_space) && !out.ends_with([' ', '\n']) && !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if text.ends_with(char::is_whitespace) && !text.trim().is_empty() {
        out.push(' ');
    }
}

fn tidy_markdown(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        blank = 0;
        out.push_str(line);
    }
    out
}

/// Removes `<script>`-like elements, whose text may look like markup.
fn strip_elements(html: &str, names: &[&str]) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    'outer: while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        for name in names {
            let open = format!("<{name}");
            let next = rest.as_bytes().get(open.len()).copied();
            if rest.len() >= open.len()
                && rest[..open.len()].eq_ignore_ascii_case(&open)
                && matches!(next, Some(b'>' | b' ' | b'\t' | b'\n' | b'/'))
            {
                let close = format!("</{name}");
                let lower = rest.to_ascii_lowercase();
                rest = match lower.find(&close) {
                    Some(c) => rest[c..]
                        .find('>')
                        .map(|g| &rest[c + g + 1..])
                        .unwrap_or(""),
                    None => "",
                };
                continue 'outer;
            }
        }
        out.push('<');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// The byte range of the first `<div>` with `class`, including the tags.
fn find_div(html: &str, class: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(i) = html[from..].find("<div") {
        let start = from + i;
        let gt = start + html[start..].find('>')?;
        let (_, _, attrs) = parse_tag(&html[start..=gt]);
        if has_class(attrs, class) {
            let mut depth = 1usize;
            let mut pos = gt + 1;
            while depth > 0 {
                let open = html[pos..].find("<div").map(|x| pos + x);
                let close = pos + html[pos..].find("</div")?;
                match open {
                    Some(o) if o < close => {
                        depth += 1;
                        pos = o + 4;
                    }
                    _ => {
                        depth -= 1;
                        pos = close + 5;
                    }
                }
            }
            let end = pos + html[pos..].find('>').map(|x| x + 1).unwrap_or(0);
            return Some((start, end));
        }
        from = gt + 1;
    }
    None
}

fn all_divs<'a>(html: &'a str, class: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut rest = html;
    while let Some((s, e)) = find_div(rest, class) {
        found.push(&rest[s..e]);
        rest = &rest[e..];
    }
    found
}

/// Text of the first `<pre>` in `html`; Codeforces splits multi-test
/// inputs into `<div class="test-example-line">` lines.
fn pre_text(html: &str) -> Option<String> {
    let start = html.find("<pre")?;
    let body_start = start + html[start..].find('>')? + 1;
    let end = body_start + html[body_start..].find("</pre")?;
    let inner = html[body_start..end]
        .replace("</div>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    let mut text = String::new();
    let mut rest = inner.as_str();
    while let Some(lt) = rest.find('<') {
        text.push_str(&rest[..lt]);
        rest = rest[lt..]
            .find('>')
            .map(|g| &rest[lt + g + 1..])
            .unwrap_or("");
    }
    text.push_str(rest);
    let text = decode_entities(&text);
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    Some(lines.join("\n").trim_matches('\n').to_owned())
}

/// A statement read from a judge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub title: Option<String>,
    pub markdown: String,
    pub samples: Vec<Sample>,
}

/// Reads a Codeforces problem page. `None` when the page has no statement
/// (e.g. a captcha or an error page).
pub fn codeforces_statement(page: &str) -> Option<Statement> {
    let page = &strip_elements(page, &["script", "style", "noscript"]);
    let (s, e) = find_div(page, "problem-statement")?;
    let block = &page[s..e];
    let title = find_div(block, "title").map(|(a, b)| {
        let t = to_markdown(&block[a..b]);
        // `A. Watermelon` → `Watermelon`.
        match t.split_once(". ") {
            Some((idx, name)) if idx.len() <= 3 => name.trim().to_owned(),
            _ => t.trim().to_owned(),
        }
    });
    let mut samples = Vec::new();
    for test in all_divs(block, "sample-test") {
        let inputs = all_divs(test, "input");
        let outputs = all_divs(test, "output");
        for (i, o) in inputs.iter().zip(outputs.iter()) {
            if let (Some(input), Some(output)) = (pre_text(i), pre_text(o)) {
                samples.push(Sample { input, output });
            }
        }
    }
    let mut text = block.to_owned();
    for class in ["header", "sample-tests"] {
        if let Some((a, b)) = find_div(&text, class) {
            text.replace_range(a..b, "");
        }
    }
    let markdown = to_markdown(&text);
    (!markdown.trim().is_empty()).then_some(Statement {
        title: title.filter(|t| !t.is_empty()),
        markdown,
        samples,
    })
}

/// Reads LeetCode's statement fragment. Examples are `Input: … Output: …`
/// blocks; they become samples the learner turns into tests.
pub fn leetcode_statement(content: &str) -> Statement {
    let markdown = to_markdown(content);
    let mut samples = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("<pre") {
        let block = &rest[start..];
        let end = block.find("</pre").map(|e| e + 5).unwrap_or(block.len());
        if let Some(text) = pre_text(&block[..end.min(block.len())].replace("</pre", "</pre>"))
            && let Some(sample) = example(&text)
        {
            samples.push(sample);
        }
        rest = &block[end.min(block.len())..];
    }
    // Newer statements write examples as `<div class="example-block">`.
    if samples.is_empty() {
        for block in all_divs(content, "example-block") {
            if let Some(sample) = example(&to_markdown(block).replace(['*', '`'], "")) {
                samples.push(sample);
            }
        }
    }
    Statement {
        title: None,
        markdown,
        samples,
    }
}

fn example(text: &str) -> Option<Sample> {
    let field = |name: &str| -> Option<String> {
        let i = text.find(name)?;
        let after = &text[i + name.len()..];
        let end = ["Output:", "Explanation:", "Input:"]
            .iter()
            .filter_map(|n| after.find(n))
            .min()
            .unwrap_or(after.len());
        Some(after[..end].trim().to_owned())
    };
    let input = field("Input:")?;
    let output = field("Output:")?;
    (!input.is_empty() && !output.is_empty()).then_some(Sample { input, output })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CF: &str = r#"<html><body><script>var x = "<div class='problem-statement'>";</script>
<div class="problemindexholder"><div class="ttypography"><div class="problem-statement">
<div class="header"><div class="title">A. Watermelon</div><div class="time-limit"><div class="property-title">time limit per test</div>1 second</div></div>
<div><p>One hot summer day Pete and his friend Billy decided to buy a watermelon. It weighs $$$w$$$ kilos (&le; 100).</p></div>
<div class="input-specification"><div class="section-title">Input</div><p>The first (and the only) input line contains integer number <span class="tex-span"><i>w</i></span>.</p></div>
<div class="output-specification"><div class="section-title">Output</div><p>Print <span class="tex-font-style-tt">YES</span> or NO.</p></div>
<div class="sample-tests"><div class="section-title">Examples</div><div class="sample-test">
<div class="input"><div class="title">Input</div><pre><div class="test-example-line test-example-line-even">2</div><div class="test-example-line">8 &amp; 9</div></pre></div>
<div class="output"><div class="title">Output</div><pre>YES
</pre></div></div></div>
<div class="note"><div class="section-title">Note</div><p>For example, the boys can divide the watermelon into two parts.</p></div>
</div></div></div></body></html>"#;

    #[test]
    fn reads_a_codeforces_page() {
        let s = codeforces_statement(CF).unwrap();
        assert_eq!(s.title.as_deref(), Some("Watermelon"));
        assert_eq!(
            s.samples,
            vec![Sample {
                input: "2\n8 & 9".into(),
                output: "YES".into()
            }]
        );
        assert!(
            s.markdown.starts_with("One hot summer day"),
            "{}",
            s.markdown
        );
        assert!(s.markdown.contains("$$$w$$$ kilos (≤ 100)"));
        assert!(s.markdown.contains("### Input\n\nThe first"));
        assert!(s.markdown.contains("integer number *w*."));
        assert!(s.markdown.contains("### Note"));
        assert!(!s.markdown.contains("time limit"));
        assert!(!s.markdown.contains("Examples"));
        assert!(!s.markdown.contains("var x"));
        assert!(codeforces_statement("<html>Just a moment...</html>").is_none());
    }

    #[test]
    fn reads_leetcode_content() {
        let html = "<p>Given an array of integers <code>nums</code>&nbsp;and an integer <code>target</code>, return <em>indices</em>.</p>\n<p>&nbsp;</p>\n<p><strong class=\"example\">Example 1:</strong></p>\n<pre>\n<strong>Input:</strong> nums = [2,7,11,15], target = 9\n<strong>Output:</strong> [0,1]\n<strong>Explanation:</strong> Because nums[0] + nums[1] == 9.\n</pre>\n<ul>\n\t<li><code>2 &lt;= nums.length &lt;= 10<sup>4</sup></code></li>\n</ul>";
        let s = leetcode_statement(html);
        assert!(
            s.markdown.starts_with(
                "Given an array of integers `nums` and an integer `target`, return *indices*."
            ),
            "{}",
            s.markdown
        );
        assert!(s.markdown.contains("```\n**Input:**") || s.markdown.contains("```\nInput:"));
        assert!(
            s.markdown.contains("- `2 <= nums.length <= 10^4`"),
            "{}",
            s.markdown
        );
        assert_eq!(
            s.samples,
            vec![Sample {
                input: "nums = [2,7,11,15], target = 9".into(),
                output: "[0,1]".into()
            }]
        );
    }

    #[test]
    fn entities() {
        assert_eq!(
            decode_entities("a &lt; b &#8804; c &#x2192; &bogus; &"),
            "a < b ≤ c → &bogus; &"
        );
    }
}
