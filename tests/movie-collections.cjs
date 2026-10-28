const fs = require('fs');
const vm = require('vm');
const html = fs.readFileSync('static/index.html', 'utf8');
const part = html.slice(html.indexOf('function needsEvidenceReview'), html.indexOf('function collectionCard('));
const ctx = { files: [], movieCollections: {}, console };
vm.createContext(ctx);
vm.runInContext(part, ctx);
function film(path, id, title, collection) { return { path, locked: false, parsed: collection ? {collection_title:collection} : {}, result:{kind:'movie',provider:'tmdb',provider_id:id,title,confidence:95,year:'2001-01-01'} }; }
ctx.files = [film('/Media/Fast/first.mkv',1,'First Fast'),film('/Media/Fast/second.mkv',2,'Second Fast'),film('/Media/Other/unrelated.mkv',3,'Fast Baking'),film('/Media/Cartoon Collection/001 - Alpha.mkv',4,'Alpha','Cartoon'),film('/Media/Cartoon Collection/002 - Beta.mkv',5,'Beta','Cartoon')];
ctx.movieCollections = {'1':{id:8,name:'Fast Collection'},'2':{id:8,name:'Fast Collection'}};
let cards=ctx.movieCards();
if(cards.length!==3)throw Error('Expected one TMDB collection, one local collection and one standalone film');
if(cards.find(c=>c.kind==='tmdbCollection')?.members.length!==2)throw Error('TMDB collection missing films');
if(cards.find(c=>c.kind==='localCollection')?.group.items.length!==2)throw Error('Local collection missing films');
if(!cards.find(c=>c.kind==='movie'&&c.film.title==='Fast Baking'))throw Error('Similar name should stay separate');
ctx.files.push(film('/Media/Other/extra-alpha.mkv',4,'Alpha'));
cards=ctx.movieCards();
if(!cards.some(c=>c.kind==='movie'&&c.film.title==='Alpha'))throw Error('A version outside a local collection must remain accessible');
console.log('Movie collection grouping checks passed');
