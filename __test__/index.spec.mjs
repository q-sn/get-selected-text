import test from 'ava';

import { getText } from '../index.js';

test('getText', (t) => {
  t.is(getText(), '');
})
