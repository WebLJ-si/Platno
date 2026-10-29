#!/usr/bin/env python3
import importlib.util
import pathlib
import unittest
ROOT=pathlib.Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('help_renderer',ROOT/'scripts/render-help.py')
renderer=importlib.util.module_from_spec(spec);spec.loader.exec_module(renderer)

class HelpRendererTests(unittest.TestCase):
    def test_headings_and_paragraphs_are_readable(self):
        result=renderer.render('# Title\n\nHello\nworld\n\n## Details\n')
        self.assertIn('<h1>Title</h1>',result);self.assertIn('<p>Hello world</p>',result);self.assertIn('<h2>Details</h2>',result)
    def test_untrusted_html_is_escaped(self):
        result=renderer.render('# <script>alert(1)</script>\n\n<img src=x onerror=alert(1)>')
        self.assertNotIn('<script>',result);self.assertNotIn('<img ',result);self.assertIn('&lt;script&gt;',result)
    def test_unicode_and_code_keep_exact_content(self):
        result=renderer.render('```sh\nŽivjo čšž\na < b && c > d\n```')
        self.assertIn('Živjo čšž',result);self.assertIn('a &lt; b &amp;&amp; c &gt; d',result)
    def test_unclosed_code_fence_is_an_error(self):
        with self.assertRaises(ValueError):renderer.render('```\nunclosed')
    def test_committed_page_matches_canonical_guide(self):
        self.assertEqual((ROOT/'static/help.html').read_text(),renderer.render((ROOT/'docs/user-guide.md').read_text()))
if __name__=='__main__':unittest.main()
