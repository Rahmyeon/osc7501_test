"""Check the offline testing guide. This does not render a browser window."""
from html.parser import HTMLParser
from pathlib import Path
import json
import re
import subprocess

source = (Path(__file__).resolve().parent.parent / 'alpha/TESTING_GUIDE.html').read_text(encoding='utf-8')
void = {'meta', 'input', 'br', 'hr', 'link', 'img'}

class Audit(HTMLParser):
    def __init__(self):
        super().__init__()
        self.stack, self.ids, self.links, self.checks, self.questions = [], set(), [], [], []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag not in void:
            self.stack.append(tag)
        if 'id' in attrs:
            assert attrs['id'] not in self.ids
            self.ids.add(attrs['id'])
        if 'href' in attrs:
            self.links.append(attrs['href'])
        if 'data-check' in attrs:
            self.checks.append(attrs['data-check'])
        if tag == 'fieldset':
            self.questions.append(attrs['data-answer'])
        assert not (tag == 'script' and 'src' in attrs)
        assert not (tag == 'link' and attrs.get('rel') == 'stylesheet')

    def handle_endtag(self, tag):
        assert self.stack.pop() == tag, tag

audit = Audit()
audit.feed(source)
assert not audit.stack
assert len(audit.checks) == len(set(audit.checks)) == 12
assert len(audit.questions) == 3
assert all(link[1:] in audit.ids for link in audit.links if link.startswith('#'))
assert not re.search(r'fetch\s*\(|@import|<iframe|<script[^>]+src=', source)
script = re.search(r'<script>(.*?)</script>', source, re.S).group(1)
harness = r'''
const assert=require('node:assert/strict');
function element(dataset={}) {return {dataset,checked:false,textContent:'',value:'test notes',events:{},addEventListener(k,v){this.events[k]=v}}}
const ids=Object.fromEntries(['count','theme','reset','download','notes'].map(k=>[k,element()]));
const checks=CHECKS.map(k=>element({check:k}));
const fields=ANSWERS.map(answer=>{const f=element({answer});f.buttons=['a','b'].map(choice=>element({choice}));f.feedback=element();f.details={open:false};f.querySelectorAll=()=>f.buttons;f.querySelector=s=>s==='.feedback'?f.feedback:f.details;return f});
const root={dataset:{}};
let clicked=false, saved;
global.document={documentElement:root,querySelector:s=>ids[s.slice(1)],querySelectorAll:s=>s==='[data-check]'?checks:fields,createElement:()=>({click(){clicked=true}})};
global.matchMedia=()=>({matches:false});
global.URL={createObjectURL(blob){saved=blob;return 'blob:test'},revokeObjectURL(){}};
global.setTimeout=fn=>fn();
{SCRIPT}
assert.equal(ids.count.textContent,'0 of 12 checks marked passed');
checks[0].checked=true;checks[0].events.change();assert.equal(ids.count.textContent,'1 of 12 checks marked passed');
ids.theme.events.click();assert.equal(root.dataset.theme,'dark');ids.theme.events.click();assert.equal(root.dataset.theme,'light');
for(const f of fields){f.buttons.find(b=>b.dataset.choice!==f.dataset.answer).events.click();assert.match(f.feedback.textContent,/Try again/);f.buttons.find(b=>b.dataset.choice===f.dataset.answer).events.click();assert.match(f.feedback.textContent,/Correct/);assert.equal(f.details.open,true)}
ids.download.events.click();assert.equal(clicked,true);
saved.text().then(text=>{assert.match(text,/PASS marked \| smoke/);assert.match(text,/test notes/);console.log('PASS: checklist, quiz feedback, theme toggle and download content')});
ids.reset.events.click();assert.equal(checks.some(x=>x.checked),false);
'''.replace('CHECKS', json.dumps(audit.checks)).replace('ANSWERS', json.dumps(audit.questions)).replace('SCRIPT', script)
subprocess.run(['node', '-'], input=harness, text=True, check=True)
print('PASS: HTML nesting, unique IDs, 12 checklist items, 3 questions and offline dependency audit')
