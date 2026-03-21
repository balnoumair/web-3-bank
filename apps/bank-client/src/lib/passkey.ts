export interface PasskeyCredentialResult {
  credentialId: string;
  publicKey: Uint8Array;
  rawId: ArrayBuffer;
}

export interface PasskeyAssertionResult {
  credentialId: string;
  rawId: ArrayBuffer;
  authenticatorData: ArrayBuffer;
  clientDataJSON: ArrayBuffer;
  signature: ArrayBuffer;
}

function bufferToBase64url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/**
 * Create a new passkey credential (registration).
 * Calls navigator.credentials.create() with P-256 algorithm for Tempo compatibility.
 */
export async function createPasskeyCredential(
  displayName: string,
): Promise<PasskeyCredentialResult> {
  const challenge = crypto.getRandomValues(new Uint8Array(32));

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
      pubKeyCredParams: [
        { alg: -7, type: 'public-key' }, // ES256 (P-256)
      ],
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
    credentialId: bufferToBase64url(credential.rawId),
    publicKey,
    rawId: credential.rawId,
  };
}

/**
 * Get an existing passkey credential (login).
 * Calls navigator.credentials.get() to trigger biometric verification.
 */
export async function getPasskeyCredential(): Promise<PasskeyAssertionResult> {
  const challenge = crypto.getRandomValues(new Uint8Array(32));

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
    credentialId: bufferToBase64url(assertion.rawId),
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
    credentialId: bufferToBase64url(assertion.rawId),
    rawId: assertion.rawId,
    authenticatorData: response.authenticatorData,
    clientDataJSON: response.clientDataJSON,
    signature: response.signature,
  };
}
