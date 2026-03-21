import { describe, it, expect } from 'vitest';
import { keccak256 } from 'viem';
import { deriveTempoAddress } from './address';

/**
 * Build a fake 91-byte SPKI-formatted P-256 public key.
 * Structure: 26-byte header | 0x04 | 32-byte x | 32-byte y
 */
function makeSPKIKey(x: Uint8Array, y: Uint8Array): Uint8Array {
  const key = new Uint8Array(91);
  key[26] = 0x04; // uncompressed point prefix
  key.set(x, 27);
  key.set(y, 59);
  return key;
}

/**
 * Build a fake 65-byte uncompressed point: 0x04 | x | y
 */
function makeUncompressedKey(x: Uint8Array, y: Uint8Array): Uint8Array {
  const key = new Uint8Array(65);
  key[0] = 0x04;
  key.set(x, 1);
  key.set(y, 33);
  return key;
}

describe('deriveTempoAddress', () => {
  const x = new Uint8Array(32).fill(0xaa);
  const y = new Uint8Array(32).fill(0xbb);

  it('derives a valid 0x-prefixed 40-char hex address from SPKI format (91 bytes)', () => {
    const key = makeSPKIKey(x, y);
    const address = deriveTempoAddress(key);
    expect(address).toMatch(/^0x[0-9a-f]{40}$/i);
  });

  it('derives a valid address from uncompressed point format (65 bytes)', () => {
    const key = makeUncompressedKey(x, y);
    const address = deriveTempoAddress(key);
    expect(address).toMatch(/^0x[0-9a-f]{40}$/i);
  });

  it('SPKI and uncompressed formats with same x,y produce the same address', () => {
    const spki = makeSPKIKey(x, y);
    const uncompressed = makeUncompressedKey(x, y);
    expect(deriveTempoAddress(spki)).toBe(deriveTempoAddress(uncompressed));
  });

  it('matches expected keccak256 derivation', () => {
    const key = makeSPKIKey(x, y);
    const coordinates = new Uint8Array(64);
    coordinates.set(x, 0);
    coordinates.set(y, 32);
    const hash = keccak256(coordinates);
    const expected = `0x${hash.slice(-40)}`;
    expect(deriveTempoAddress(key)).toBe(expected);
  });

  it('different keys produce different addresses', () => {
    const key1 = makeSPKIKey(new Uint8Array(32).fill(0x01), new Uint8Array(32).fill(0x02));
    const key2 = makeSPKIKey(new Uint8Array(32).fill(0x03), new Uint8Array(32).fill(0x04));
    expect(deriveTempoAddress(key1)).not.toBe(deriveTempoAddress(key2));
  });

  it('throws for unexpected key length', () => {
    const bad = new Uint8Array(32);
    expect(() => deriveTempoAddress(bad)).toThrow('Unexpected public key format');
    expect(() => deriveTempoAddress(bad)).toThrow('length=32');
  });

  it('throws for 65-byte key that does not start with 0x04', () => {
    const bad = new Uint8Array(65); // first byte is 0x00, not 0x04
    expect(() => deriveTempoAddress(bad)).toThrow('Unexpected public key format');
  });
});
