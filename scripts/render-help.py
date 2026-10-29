#!/usr/bin/env python3
"""Render the supported Markdown subset into the bundled help page; --check detects drift."""
import argparse
import html
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]

def render(markdown):
    output, paragraph, in_code, code = [], [], False, []
    def flush():
        if paragraph:
            output.append('<p>' + html.escape(' '.join(paragraph)) + '</p>')
            paragraph.clear()
    for line in markdown.splitlines():
        if line.startswith('```'):
            flush()
            if in_code: output.append('<pre><code>' + html.escape('\n'.join(code)) + '</code></pre>'); code=[]
            in_code = not in_code
        elif in_code: code.append(line)
        elif not line.strip(): flush()
        elif match := re.match(r'^(#{1,3}) (.+)$',line):
            flush(); level=len(match[1]);output.append(f'<h{level}>{html.escape(match[2])}</h{level}>')
        else: paragraph.append(line)
    flush()
    if in_code: raise ValueError('Unclosed Markdown code fence')
    return '<!doctype html>\n<html lang="sl"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>PlexV2 · Navodila</title><style>body{margin:0;background:#0c1017;color:#eef2f7;font:17px/1.65 system-ui}main{max-width:900px;margin:auto;padding:24px}a{color:#e6ad55}h1,h2,h3{line-height:1.25}h2{margin-top:2.5em;border-top:1px solid #364255;padding-top:1em}pre{padding:16px;background:#171d28;border-radius:8px;overflow:auto}p{overflow-wrap:anywhere}:focus-visible{outline:3px solid #e6ad55}</style></head><body><main><a href="/">← Nazaj v knjižnico</a>\n'+'\n'.join(output)+'\n</main></body></html>\n'

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args()
    result=render((ROOT/'docs/user-guide.sl.md').read_text())
    target=ROOT/'static/help.html'
    if args.check:
        if not target.exists() or target.read_text()!=result: raise SystemExit('Help page drift: run python3 scripts/render-help.py')
        print('Bundled help matches the user guide')
    else: target.write_text(result)
if __name__=='__main__': main()
