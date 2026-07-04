import { keccak256 } from "viem";

/** Decode base64url (browser WebAuthn) into bytes. */
export function base64urlToBuffer(b64url: string): Uint8Array {
  const base64 = b64url.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  return Uint8Array.from(atob(padded), (c) => c.charCodeAt(0));
}

/** Encode bytes as base64url. */
export function bufferToBase64url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Accept hex (legacy frontend) or base64url public key encodings. */
export function decodePublicKey(input: string): Uint8Array {
  const trimmed = input.trim();
  if (/^[0-9a-fA-F]+$/.test(trimmed) && trimmed.length % 2 === 0) {
    return Uint8Array.from(trimmed.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)));
  }
  return base64urlToBuffer(trimmed);
}

/**
 * Derive a Tempo address from a P-256 (ES256) public key in SPKI or uncompressed form.
 */
export function deriveTempoAddress(publicKey: Uint8Array): `0x${string}` {
  let rawPoint: Uint8Array;

  if (publicKey.length === 91) {
    rawPoint = publicKey.slice(26);
  } else if (publicKey.length === 65 && publicKey[0] === 0x04) {
    rawPoint = publicKey;
  } else {
    throw new Error(
      `Unexpected public key format: length=${publicKey.length}, first byte=0x${publicKey[0]?.toString(16)}`,
    );
  }

  const coordinates = rawPoint.slice(1);
  const hash = keccak256(
    `0x${Buffer.from(coordinates).toString("hex")}` as `0x${string}`,
  );
  return `0x${hash.slice(-40)}` as `0x${string}`;
}
