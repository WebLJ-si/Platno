const fs = require('node:fs');
const vm = require('node:vm');
for (const file of ['static/index.html']) {
  const html = fs.readFileSync(file, 'utf8');
  let count = 0;
  for (const [, attrs, source] of html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)) {
    if (/src=|application\/json/.test(attrs)) continue;
    new vm.Script(source, { filename: file });
    count++;
  }
  console.log(`${file}: ${count} scripts parse`);
}
