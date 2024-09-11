import test from 'ava';

import { getText } from '../index.js';

test('getText', async (t) => {
  t.is(await getText(), '');
})
