import { base64urlToBuffer, bufferToBase64url } from '@repo/tempo-crypto';

export interface PasskeyCredentialResult {
  credentialId: string;
  publicKey: Uint8Array;
  rawId: ArrayBuffer;
  clientDataJSON: ArrayBuffer;
  attestationObject: ArrayBuffer;
}

export interface PasskeyAssertionResult {
  credentialId: string;
  rawId: ArrayBuffer;
  authenticatorData: ArrayBuffer;
  clientDataJSON: ArrayBuffer;
  signature: ArrayBuffer;
}

function bufferToBase64urlFromArrayBuffer(buffer: ArrayBuffer): string {
  return bufferToBase64url(new Uint8Array(buffer));
}

/**
 * Create a new passkey credential (registration) using a BFF-issued challenge.
 */
export async function createPasskeyCredential(
  displayName: string,
  challengeB64url: string,
): Promise<PasskeyCredentialResult> {
  const challenge = base64urlToBuffer(challengeB64url);

  const credential = (await navigator.credentials.create({
    publicKey: {
      challenge,
      rp: {
        name: 'Web3Bank',
        id: window.location.hostname,
      },
      user: {
        id: crypto.getRandomValues(new Uint8Array(16)),
        name: displayName,
        displayName,
      },
      pubKeyCredParams: [{ alg: -7, type: 'public-key' }],
      authenticatorSelection: {
        authenticatorAttachment: 'platform',
        residentKey: 'required',
        userVerification: 'required',
      },
      attestation: 'none',
      timeout: 60000,
    },
  })) as PublicKeyCredential | null;

  if (!credential) {
    throw new Error('Passkey creation was cancelled');
  }

  const response = credential.response as AuthenticatorAttestationResponse;
  const publicKey = new Uint8Array(response.getPublicKey()!);

  return {
    credentialId: bufferToBase64urlFromArrayBuffer(credential.rawId),
    publicKey,
    rawId: credential.rawId,
    clientDataJSON: response.clientDataJSON,
    attestationObject: response.attestationObject,
  };
}

/**
 * Get an existing passkey credential (login) using a BFF-issued challenge.
 */
export async function getPasskeyCredential(
  challengeB64url: string,
): Promise<PasskeyAssertionResult> {
  const challenge = base64urlToBuffer(challengeB64url);

  const assertion = (await navigator.credentials.get({
    publicKey: {
      challenge,
      rpId: window.location.hostname,
      userVerification: 'required',
      timeout: 60000,
    },
  })) as PublicKeyCredential | null;

  if (!assertion) {
    throw new Error('Passkey authentication was cancelled');
  }

  const response = assertion.response as AuthenticatorAssertionResponse;

  return {
    credentialId: bufferToBase64urlFromArrayBuffer(assertion.rawId),
    rawId: assertion.rawId,
    authenticatorData: response.authenticatorData,
    clientDataJSON: response.clientDataJSON,
    signature: response.signature,
  };
}

/**
 * Sign a transaction challenge with an existing passkey.
 * Used for on-chain transaction signing (EIP-2718 passkey tx).
 */
export async function signWithPasskey(
  challenge: Uint8Array,
): Promise<PasskeyAssertionResult> {
  const assertion = (await navigator.credentials.get({
    publicKey: {
      challenge,
      rpId: window.location.hostname,
      userVerification: 'required',
      timeout: 60000,
    },
  })) as PublicKeyCredential | null;

  if (!assertion) {
    throw new Error('Transaction signing was cancelled');
  }

  const response = assertion.response as AuthenticatorAssertionResponse;

  return {
    credentialId: bufferToBase64urlFromArrayBuffer(assertion.rawId),
    rawId: assertion.rawId,
    authenticatorData: response.authenticatorData,
    clientDataJSON: response.clientDataJSON,
    signature: response.signature,
  };
}

export { bufferToBase64urlFromArrayBuffer as webAuthnFieldToBase64url };
