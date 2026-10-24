// What does the awesome-dsh-plugin catalog require from a submission?
const UA = 'samplesearch-probe/0.1';

async function text(url) {
  const r = await fetch(url, { headers: { 'user-agent': UA }, signal: AbortSignal.timeout(20000) });
  return { status: r.status, body: await r.text() };
}

const contrib = await text(
  'https://raw.githubusercontent.com/awesome-dsh-plugin/awesome-dsh-plugin/main/contributing.md',
);
console.log(`=== contributing.md status=${contrib.status} ===`);
console.log(contrib.body.slice(0, 4000));

const cat = await text('https://awesome-dsh-plugin.com/plugins.json');
console.log(`\n=== plugins.json status=${cat.status} bytes=${cat.body.length} ===`);
try {
  const list = JSON.parse(cat.body);
  const arr = Array.isArray(list) ? list : list.plugins ?? [];
  console.log('entries:', arr.length);
  console.log('first entry:\n', JSON.stringify(arr[0], null, 2).slice(0, 1200));
} catch {
  console.log(cat.body.slice(0, 800));
}
