'use strict';

const assert = require('assert');
const { parseChecksum } = require('../scripts/lib/checksum');

const hash = 'a'.repeat(64);
assert.strictEqual(parseChecksum(`${hash}  tfy-0.1.0-darwin-arm64.tar.gz\n`, 'tfy-0.1.0-darwin-arm64.tar.gz'), hash);
assert.strictEqual(parseChecksum(`${hash} *tfy-0.1.0-darwin-arm64.tar.gz\n`, 'tfy-0.1.0-darwin-arm64.tar.gz'), hash);
assert.strictEqual(parseChecksum(`${hash}\n`, 'anything.tar.gz'), hash);
assert.throws(() => parseChecksum(`${hash}  other-tfy.tar.gz\n`, 'tfy.tar.gz'), /No checksum entry/);
assert.throws(() => parseChecksum(`${hash}  tfy.tar.gz.extra\n`, 'tfy.tar.gz'), /No checksum entry/);
assert.throws(() => parseChecksum('not-a-hash tfy.tar.gz\n', 'tfy.tar.gz'), /No checksum entry/);
