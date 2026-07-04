import { getWebAuthnConfig } from "./config.js";

type ChallengeEntry = { expiresAt: number };

const challenges = new Map<string, ChallengeEntry>();

export function issueChallenge(): string {
  const challenge = Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString(
    "base64url",
  );
  const { challengeTtlMs } = getWebAuthnConfig();
  challenges.set(challenge, { expiresAt: Date.now() + challengeTtlMs });
  return challenge;
}

/** Returns true and burns the challenge when valid. */
export function consumeChallenge(challenge: string): boolean {
  const entry = challenges.get(challenge);
  if (!entry) {
    return false;
  }
  challenges.delete(challenge);
  return Date.now() <= entry.expiresAt;
}

/** Test helper — clears all outstanding challenges. */
export function resetChallengesForTests(): void {
  challenges.clear();
}
