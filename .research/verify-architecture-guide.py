"""Verify the offline lesson's structure, references, contrast and interactions.

This uses an HTML parser and a small DOM substitute, not a browser renderer.
"""
from html.parser import HTMLParser
from pathlib import Path
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
HTML = ROOT / "wezterm-architecture-guide.html"
source = HTML.read_text(encoding="utf-8")
void = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}


class Audit(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack = []
        self.ids = {}
        self.hrefs = []
        self.questions = []
        self.button_ids = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag not in void:
            self.stack.append(tag)
        if "id" in attrs:
            assert attrs["id"] not in self.ids, f"Duplicate ID: {attrs['id']}"
            self.ids[attrs["id"]] = tag
            if tag == "button":
                self.button_ids.append(attrs["id"])
        if "href" in attrs:
            self.hrefs.append(attrs["href"])
        assert not (tag == "script" and "src" in attrs), "External script"
        assert not (tag == "link" and attrs.get("rel") == "stylesheet"), "External stylesheet"
        assert not (tag in {"img", "iframe", "audio", "video"} and "src" in attrs), "Asset needs review"
        if tag == "fieldset":
            self.questions.append({"answer": attrs["data-answer"], "feedback": attrs["data-feedback"], "choices": []})
        if tag == "button" and "data-choice" in attrs:
            self.questions[-1]["choices"].append(attrs["data-choice"])

    def handle_endtag(self, tag):
        assert self.stack and self.stack[-1] == tag, f"Mismatched closing {tag}, stack={self.stack}"
        self.stack.pop()


audit = Audit()
audit.feed(source)
assert not audit.stack, f"Unclosed tags: {audit.stack}"
assert source.lower().startswith("<!doctype html>")
assert all(tag in source for tag in ["<html", "<head>", "<body>", "<style>", "<script>"])
assert len(audit.questions) == 4
assert not re.search(r"\bfetch\s*\(|\bimport\s*\(|@import|url\s*\(", source)
for href in audit.hrefs:
    if href.startswith("#"):
        assert href[1:] in audit.ids, f"Missing anchor {href}"
    elif not href.startswith("https://"):
        assert (ROOT / href.split("#")[0]).is_file(), f"Missing local target {href}"

guide = (ROOT / "WEZTERM_DESIGN_GUIDE.md").read_text(encoding="utf-8")
links = set(re.findall(r"https://github\.com/wezterm/wezterm/blob/([a-f0-9]{40})/([^\s\"<)]+)#L(\d+)", source + guide))
for revision, filename, line in links:
    assert revision == "e2a98e1b8af156988ad0ac778658ac27b60a55fb"
    lines = (ROOT / "wezterm" / filename).read_text(encoding="utf-8").splitlines()
    assert 0 < int(line) <= len(lines), f"Invalid source line {filename}:{line}"
commits = set(re.findall(r"https://github\.com/wezterm/wezterm/commit/([a-f0-9]{40})", source + guide))
for commit in commits:
    result = subprocess.run(["git", "show", "-s", "--format=%an", commit], cwd=ROOT / "wezterm", capture_output=True, text=True, check=True)
    assert result.stdout.strip() == "Wez Furlong", f"Unexpected author: {commit}"
for name in ["WEZTERM_DESIGN_GUIDE.md", "wezterm-architecture-guide.html", "AGENTS.md"]:
    text = (ROOT / name).read_text(encoding="utf-8")
    assert not re.search("[\u2013\u2014\u201c\u201d]", text), f"Style punctuation: {name}"


def luminance(color):
    channels = [int(color[index:index + 2], 16) / 255 for index in [1, 3, 5]]
    channels = [value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4 for value in channels]
    return sum(value * weight for value, weight in zip(channels, [0.2126, 0.7152, 0.0722]))


def contrast(a, b):
    high, low = sorted([luminance(a), luminance(b)], reverse=True)
    return (high + 0.05) / (low + 0.05)


palettes = []
for theme in ["light", "dark"]:
    block = re.search(r':root\[data-theme="' + theme + r'"\]\{([^}]+)\}', source).group(1)
    palette = dict(re.findall(r"--([a-z]+):(#[0-9a-f]{6})", block))
    ratios = [contrast(palette[foreground], palette[background]) for foreground in ["ink", "muted", "accent", "good", "bad", "warn"] for background in ["bg", "paper"]]
    assert min(ratios) >= 7, f"{theme} minimum contrast {min(ratios):.2f}"
    palettes.append(f"{theme} text minimum {min(ratios):.2f}:1")

script = re.search(r"<script>([\s\S]*?)</script>", source).group(1)
payload = {"script": script, "ids": list(audit.ids), "questions": audit.questions}
node_test = r"""
const fs = require('fs');
const vm = require('vm');
const assert = require('assert/strict');
const payload = JSON.parse(fs.readFileSync(0, 'utf8'));
function element(dataset = {}) {
  return {
    dataset, textContent: '', className: '', listeners: {}, attributes: {},
    addEventListener(type, fn) { this.listeners[type] = fn; },
    setAttribute(key, value) { this.attributes[key] = value; },
    removeAttribute(key) { delete this.attributes[key]; },
    click() { assert.equal(typeof this.listeners.click, 'function'); this.listeners.click(); }
  };
}
function run(dark) {
  const nodes = Object.fromEntries(payload.ids.map(id => [id, element()]));
  const questions = payload.questions.map(data => {
    const q = element({answer: data.answer, feedback: data.feedback});
    q.buttons = data.choices.map(choice => element({choice}));
    q.feedback = element();
    q.querySelector = sel => { assert.equal(sel, '.feedback'); return q.feedback; };
    q.querySelectorAll = sel => { assert.equal(sel, '[data-choice]'); return q.buttons; };
    return q;
  });
  const root = element();
  const preference = {matches: dark, addEventListener(type, fn) { this.onchange = fn; }};
  const document = {documentElement: root, getElementById(id) { assert.ok(nodes[id], id); return nodes[id]; }, querySelectorAll(sel) { assert.equal(sel, '.question'); return questions; }};
  vm.runInNewContext(payload.script, {document, window: {matchMedia() { return preference; }}});
  assert.equal(nodes.theme.textContent, dark ? 'Use light theme' : 'Use dark theme');
  nodes.theme.click();
  assert.equal(root.dataset.theme, dark ? 'light' : 'dark');
  preference.matches = !dark;
  preference.onchange();
  assert.equal(root.dataset.theme, dark ? 'light' : 'dark');
  nodes.theme.click();
  assert.equal(root.dataset.theme, dark ? 'dark' : 'light');

  nodes['demo-reset'].click();
  assert.equal(nodes['owner-value'].textContent, 'None');
  assert.equal(nodes['consumer-value'].textContent, 'None');
  nodes['toggle-consumer'].click();
  nodes.send42.click();
  assert.equal(nodes['owner-value'].textContent, 'Percentage(42)');
  assert.equal(nodes['consumer-value'].textContent, 'None');
  nodes.snapshot.click();
  assert.match(nodes['demo-log'].textContent, /Reconnect/);
  nodes['toggle-consumer'].click();
  assert.equal(nodes['consumer-value'].textContent, 'None');
  nodes.send42.click();
  assert.equal(nodes['change-count'].textContent, 'Changes emitted: 1');
  assert.equal(nodes['consumer-value'].textContent, 'None');
  nodes.snapshot.click();
  assert.equal(nodes['consumer-value'].textContent, 'Percentage(42)');
  nodes.send80.click();
  assert.equal(nodes['consumer-value'].textContent, 'Percentage(80)');
  assert.equal(nodes['change-count'].textContent, 'Changes emitted: 2');
  nodes['demo-reset'].click();
  assert.equal(nodes['change-count'].textContent, 'Changes emitted: 0');
  assert.equal(nodes['consumer-value'].textContent, 'None');

  questions[0].buttons.find(b => b.dataset.choice !== questions[0].dataset.answer).click();
  assert.match(questions[0].feedback.textContent, /^Try again/);
  assert.equal(nodes['quiz-score'].textContent, '0 of 4 correct');
  for (const q of questions) {
    const correct = q.buttons.find(b => b.dataset.choice === q.dataset.answer);
    correct.click();
    assert.equal(correct.attributes['aria-pressed'], 'true');
    assert.match(q.feedback.textContent, /^Correct/);
  }
  assert.equal(nodes['quiz-score'].textContent, '4 of 4 correct');
  questions[0].buttons.find(b => b.dataset.choice !== questions[0].dataset.answer).click();
  assert.equal(nodes['quiz-score'].textContent, '3 of 4 correct');
  nodes['quiz-reset'].click();
  assert.equal(nodes['quiz-score'].textContent, '0 of 4 correct');
  for (const q of questions) {
    assert.equal(q.feedback.textContent, '');
    assert.ok(q.buttons.every(b => !('aria-pressed' in b.attributes)));
  }
}
run(false); run(true);
process.stdout.write('Interaction checks passed for both system themes.\n');
"""
result = subprocess.run(["node", "-e", node_test], input=json.dumps(payload), text=True, capture_output=True, check=True)
print(result.stdout.strip())
print(f"Structure, offline dependencies and local links passed; {len(links)} pinned source locations and {len(commits)} Wez-authored commits checked.")
print("; ".join(palettes))
print("No browser rendering was performed.")
