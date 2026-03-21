import { keccak256 } from 'viem';

/**
 * Derive a Tempo address from a P-256 (ES256) public key.
 *
 * The public key from WebAuthn is in COSE format. We extract the
 * uncompressed x,y coordinates and hash them with keccak256, then
 * take the last 20 bytes as the address.
 *
 * Note: The exact derivation algorithm depends on Tempo's spec.
 * This implementation follows the standard EVM pattern adapted for P-256.
 */
export function deriveTempoAddress(publicKey: Uint8Array): `0x${string}` {
  // The WebAuthn public key from getPublicKey() is in SubjectPublicKeyInfo (SPKI) format.
  // For P-256, the SPKI wraps the uncompressed point (0x04 || x || y).
  // The SPKI header for P-256 is 26 bytes, so the raw point starts at offset 26.
  // Total SPKI length = 91 bytes (26 header + 1 prefix + 32 x + 32 y)

  let rawPoint: Uint8Array;

  if (publicKey.length === 91) {
    // SPKI format — skip the 26-byte header
    rawPoint = publicKey.slice(26);
  } else if (publicKey.length === 65 && publicKey[0] === 0x04) {
    // Already uncompressed point format
    rawPoint = publicKey;
  } else {
    throw new Error(
      `Unexpected public key format: length=${publicKey.length}, first byte=0x${publicKey[0]?.toString(16)}`,
    );
  }

  // Skip the 0x04 uncompressed prefix, hash x || y (64 bytes)
  const coordinates = rawPoint.slice(1);
  const hash = keccak256(coordinates as `0x${string}`);

  // Take last 20 bytes (40 hex chars) as the address
  return `0x${hash.slice(-40)}` as `0x${string}`;
}
