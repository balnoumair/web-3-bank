import { describe, expect, it } from "vitest";
import { keccak256 } from "viem";
import { deriveTempoAddress } from "./index.js";

function makeSPKIKey(x: Uint8Array, y: Uint8Array): Uint8Array {
  const key = new Uint8Array(91);
  key[26] = 0x04;
  key.set(x, 27);
  key.set(y, 59);
  return key;
}

describe("deriveTempoAddress", () => {
  const x = new Uint8Array(32).fill(0xaa);
  const y = new Uint8Array(32).fill(0xbb);

  it("matches keccak256(x || y) last 20 bytes", () => {
    const key = makeSPKIKey(x, y);
    const expected = `0x${keccak256(new Uint8Array([...x, ...y]) as `0x${string}`).slice(-40)}`;
    expect(deriveTempoAddress(key)).toBe(expected);
  });
});
