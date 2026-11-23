import { readFile } from 'node:fs/promises';

const dist = new URL('../dist/', import.meta.url);
const guidePath = 'guides/roomeq-cli/index.html';
const guide = await readFile(new URL(guidePath, dist), 'utf8');
const origin = 'https://sotf.spinorama.org';
const page = new URL(guidePath, origin);
const archives = [...guide.matchAll(/<a\b[^>]*?\shref=(["'])([^"']+)\1/gi)]
  .map((match) => new URL(match[2], page))
  .filter((url) => url.pathname.endsWith('/roomeq.zip'));
if (!archives.length || archives.some((url) => url.origin !== origin)) {
  throw new Error('The built RoomEQ CLI guide needs a download link on this site.');
}

const expected = await readFile(new URL('../../../autoeq/examples/roomeq.zip', import.meta.url));
for (const url of archives) {
  const artifact = await readFile(new URL(url.pathname.slice(1), dist));
  if (!artifact.equals(expected)) {
    throw new Error(`The guide links to ${url.pathname}, whose bytes differ from the RoomEQ release example.`);
  }
}
console.log(`Verified RoomEQ guide download (${expected.length} bytes).`);
