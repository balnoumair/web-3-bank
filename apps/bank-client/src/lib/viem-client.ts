import { createPublicClient, http } from 'viem';
import { tempo } from '~/config/chain';
import { signWithPasskey } from './passkey';

export const publicClient = createPublicClient({
  chain: tempo,
  transport: http(),
});

/**
 * Sign and broadcast an EIP-2718 passkey transaction on Tempo.
 *
 * This function encapsulates the Tempo-specific passkey transaction format:
 * 1. Estimates gas via the public client
 * 2. Builds the unsigned transaction
 * 3. Presents the tx hash as a WebAuthn challenge (triggers biometric prompt)
 * 4. Packages the WebAuthn assertion into the EIP-2718 signature envelope
 * 5. Broadcasts via eth_sendRawTransaction
 *
 * Note: The exact EIP-2718 type byte and signature encoding depend on
 * Tempo's specification. This implementation will be refined once the
 * Tempo SDK/docs are available. The key architectural guarantee is that
 * all Tempo-specific encoding is contained within this single function.
 */
export async function signAndSendPasskeyTx(params: {
  from: `0x${string}`;
  to: `0x${string}`;
  data: `0x${string}`;
  value?: bigint;
}): Promise<`0x${string}`> {
  const { from, to, data, value = 0n } = params;

  // 1. Estimate gas
  const gasEstimate = await publicClient.estimateGas({
    account: from,
    to,
    data,
    value,
  });

  // 2. Get current gas price and nonce
  const [gasPrice, nonce] = await Promise.all([
    publicClient.getGasPrice(),
    publicClient.getTransactionCount({ address: from }),
  ]);

  // 3. Build the unsigned tx payload
  const unsignedTx = {
    chainId: tempo.id,
    nonce,
    to,
    data,
    value,
    gas: gasEstimate,
    gasPrice,
  };

  // 4. Compute the signing hash (tx hash used as WebAuthn challenge)
  // For now, we serialize the tx fields and hash them.
  // The exact serialization depends on Tempo's EIP-2718 type definition.
  const txPayload = new TextEncoder().encode(JSON.stringify(unsignedTx));
  const hashBuffer = await crypto.subtle.digest('SHA-256', txPayload);
  const challenge = new Uint8Array(hashBuffer);

  // 5. Sign with passkey (triggers biometric prompt)
  const assertion = await signWithPasskey(challenge);

  // 6. Broadcast the signed transaction
  // Tempo's RPC accepts a custom method or wraps the WebAuthn signature
  // into the standard eth_sendRawTransaction format.
  // This is a placeholder — the actual RPC call format will depend on Tempo's spec.
  const txHash = await publicClient.request({
    method: 'eth_sendTransaction' as any,
    params: [
      {
        from,
        to,
        data,
        value: `0x${value.toString(16)}`,
        gas: `0x${gasEstimate.toString(16)}`,
        gasPrice: `0x${gasPrice.toString(16)}`,
        nonce: `0x${nonce.toString(16)}`,
        // Tempo-specific: WebAuthn signature fields
        authData: bufferToHex(assertion.authenticatorData),
        clientDataJSON: bufferToHex(assertion.clientDataJSON),
        signature: bufferToHex(assertion.signature),
        credentialId: assertion.credentialId,
      } as any,
    ],
  });

  return txHash as `0x${string}`;
}

function bufferToHex(buffer: ArrayBuffer): `0x${string}` {
  const bytes = new Uint8Array(buffer);
  let hex = '0x';
  for (const byte of bytes) {
    hex += byte.toString(16).padStart(2, '0');
  }
  return hex as `0x${string}`;
}

/**
 * Wait for a transaction to be confirmed.
 */
export async function waitForTx(hash: `0x${string}`) {
  return publicClient.waitForTransactionReceipt({ hash });
}
