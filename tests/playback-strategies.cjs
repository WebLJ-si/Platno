const fs = require('fs');
const vm = require('vm');
const assert = require('assert');
const html = fs.readFileSync('static/index.html', 'utf8');
const start = html.indexOf('function chooseStreamMode(');
const end = html.indexOf('\nasync function startPlayback', start);
assert(start >= 0 && end > start, 'player strategy function must exist');
const context = {};
vm.createContext(context);
vm.runInContext(html.slice(start, end), context);
const choose = context.chooseStreamMode;
assert.equal(choose(true, true, true, false), 'direct', 'compatible originals should use range streaming');
assert.equal(choose(false, true, true, false), 'mse-hls', 'MSE should be preferred when both report support');
assert.equal(choose(false, true, false, false), 'native-hls', 'native-only browsers should use HLS');
assert.equal(choose(false, false, true, false), 'mse-hls', 'MSE browsers should use HLS.js');
assert.equal(choose(false, false, false, false), 'progressive', 'legacy browsers retain a fallback');
assert.equal(choose(false, false, true, true), 'progressive', 'failed HLS can fall back');
const recommendationStart = html.indexOf('function recommendationTerms(');
const recommendationEnd = html.indexOf('function loadContinueWatching(', recommendationStart);
assert(recommendationStart >= 0 && recommendationEnd > recommendationStart, 'recommendation algorithm must exist');
vm.runInContext(html.slice(recommendationStart, recommendationEnd), context);
const history = [{ completed: false, position: 31, duration: 60, result: { kind: 'movie', provider_id: 7, title: 'Space Voyage', description: 'A crew explores a distant planet and discovers alien life.' } }];
const movies = [
  { title: 'Alien Planet', provider_id: 8, description: 'Explorers find alien life on a distant planet.' },
  { title: 'Cooking at Home', provider_id: 9, description: 'A family learns to prepare meals together.' },
];
const recommendations = context.localRecommendations(history, movies, []);
assert.equal(recommendations[0]?.item.title, 'Alien Planet', 'recommendations should use shared story details');
assert(!recommendations.some(item => item.item.provider_id === 7), 'watched items should not be recommended again');
console.log('Playback strategy and recommendation checks passed');
