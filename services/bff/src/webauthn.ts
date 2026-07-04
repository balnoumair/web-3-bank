import {
  verifyAuthenticationResponse,
  verifyRegistrationResponse,
  type VerifiedAuthenticationResponse,
  type VerifiedRegistrationResponse,
} from "@simplewebauthn/server";
import { getWebAuthnConfig } from "./config.js";

export type WebAuthnAssertionInput = {
  credentialId: string;
  authenticatorData: string;
  clientDataJSON: string;
  signature: string;
};

export type WebAuthnAttestationInput = {
  credentialId: string;
  clientDataJSON: string;
  attestationObject: string;
};

function webAuthnOptions(challenge: string) {
  const { rpID, origin } = getWebAuthnConfig();
  return { expectedChallenge: challenge, expectedOrigin: origin, expectedRPID: rpID };
}

export async function verifyRegistration(
  attestation: WebAuthnAttestationInput,
  challenge: string,
): Promise<VerifiedRegistrationResponse> {
  return verifyRegistrationResponse({
    response: {
      id: attestation.credentialId,
      rawId: attestation.credentialId,
      type: "public-key",
      clientExtensionResults: {},
      response: {
        clientDataJSON: attestation.clientDataJSON,
        attestationObject: attestation.attestationObject,
      },
    },
    ...webAuthnOptions(challenge),
    requireUserVerification: true,
  });
}

export async function verifyAssertion(
  assertion: WebAuthnAssertionInput,
  challenge: string,
  credential: { id: string; publicKey: Uint8Array; counter?: number },
): Promise<VerifiedAuthenticationResponse> {
  return verifyAuthenticationResponse({
    response: {
      id: assertion.credentialId,
      rawId: assertion.credentialId,
      type: "public-key",
      clientExtensionResults: {},
      response: {
        authenticatorData: assertion.authenticatorData,
        clientDataJSON: assertion.clientDataJSON,
        signature: assertion.signature,
      },
    },
    ...webAuthnOptions(challenge),
    credential: {
      id: assertion.credentialId,
      publicKey: new Uint8Array(credential.publicKey),
      counter: credential.counter ?? 0,
    },
    requireUserVerification: true,
  });
}

export function challengeFromClientData(clientDataJSON: string): string {
  const parsed = JSON.parse(
    Buffer.from(clientDataJSON, "base64url").toString("utf8"),
  ) as { challenge?: string };
  if (!parsed.challenge || typeof parsed.challenge !== "string") {
    throw new Error("clientDataJSON missing challenge");
  }
  return parsed.challenge;
}
