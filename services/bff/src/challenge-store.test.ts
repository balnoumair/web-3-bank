import { describe, expect, it } from "bun:test";
import { consumeChallenge, issueChallenge, resetChallengesForTests } from "./challenge-store.js";

describe("challenge-store", () => {
  it("issues and consumes a challenge once", () => {
    resetChallengesForTests();
    const challenge = issueChallenge();
    expect(consumeChallenge(challenge)).toBe(true);
    expect(consumeChallenge(challenge)).toBe(false);
  });

  it("rejects unknown challenges", () => {
    resetChallengesForTests();
    expect(consumeChallenge("not-a-real-challenge")).toBe(false);
  });
});
