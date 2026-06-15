'use strict';

function parseChecksum(text, asset) {
  const expectedName = String(asset);
  for (const line of String(text).split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const parts = trimmed.split(/\s+/);
    const hash = parts[0];
    if (!/^[a-fA-F0-9]{64}$/.test(hash || '')) continue;
    if (parts.length === 1) return hash.toLowerCase();
    const filename = parts.slice(1).join(' ').replace(/^\*/, '');
    if (filename === expectedName) return hash.toLowerCase();
  }
  throw new Error(`No checksum entry found for ${asset}`);
}

module.exports = { parseChecksum };
