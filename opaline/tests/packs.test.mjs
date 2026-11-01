import assert from 'node:assert/strict';
import { splitPacks } from '../ui/packs.js';

const P = (id, installed, supported = true) => ({ id, name: id.toUpperCase(), installed, supported });
const all = [P('a', false, false), P('b', true), P('c', false), P('d', true), P('e', false, false), P('f', false)];
const { mine, avail } = splitPacks(all);
assert.deepEqual(mine.map((p) => p.id), ['b', 'd']);
assert.deepEqual(avail.map((p) => p.id), ['c', 'f', 'a', 'e']); // pris en charge d'abord, ordre du catalogue conservé
assert.deepEqual(splitPacks(all, 'c').avail.map((p) => p.id), ['c']);
assert.deepEqual(splitPacks([], 'x'), { mine: [], avail: [] });
console.log('ok packs');
