import { afterEach, describe, expect, it } from "bun:test";
import { makeMutationUseCases } from "./mutations.js";
import {
  consumeChallenge,
  resetChallengesForTests,
} from "../challenge-store.js";
import type { CredentialAuthRecord, IUserService } from "../domain/ports/user-service.js";

const originalDevMode = process.env.BFF_DEV_MODE;
const originalJwtSecret = process.env.JWT_SECRET;

afterEach(() => {
  resetChallengesForTests();
  if (originalDevMode === undefined) {
    delete process.env.BFF_DEV_MODE;
  } else {
    process.env.BFF_DEV_MODE = originalDevMode;
  }
  if (originalJwtSecret === undefined) {
    delete process.env.JWT_SECRET;
  } else {
    process.env.JWT_SECRET = originalJwtSecret;
  }
});

function mockUserService(
  overrides: Partial<IUserService> = {},
): IUserService {
  const record: CredentialAuthRecord = {
    userId: "user-1",
    displayName: "Alice",
    status: "active",
    tempoAddress: "0xaaaa111111111111111111111111111111111111",
    username: "",
    publicKey: Buffer.from("stored-key"),
    revoked: false,
  };

  return {
    createUser: async () => ({ userId: "user-1" }),
    getUserByAddress: async () => record,
    getUserByCredentialId: async () => record,
    addCredential: async () => ({ credentialId: "new-cred" }),
    setUsername: async () => record,
    getUserByUsername: async () => record,
    getUserHomeChain: async () => ({ found: false }),
    listCredentials: async () => [],
    ...overrides,
  };
}

describe("auth mutations", () => {
  it("rejects legacy authenticate outside dev mode", async () => {
    delete process.env.BFF_DEV_MODE;
    process.env.JWT_SECRET = "test-secret";
    const mutations = makeMutationUseCases(mockUserService());

    await expect(
      mutations.authenticateLegacy({ credentialId: "abc" }),
    ).rejects.toThrow("Legacy authenticate is disabled");
  });

  it("rejects authenticate with replayed challenge", async () => {
    process.env.JWT_SECRET = "test-secret";
    const mutations = makeMutationUseCases(mockUserService());
    const { challenge } = mutations.requestChallenge();
    consumeChallenge(challenge);

    const assertion = {
      credentialId: "abc",
      authenticatorData: "aa",
      clientDataJSON: Buffer.from(
        JSON.stringify({ challenge, type: "webauthn.get" }),
      ).toString("base64url"),
      signature: "sig",
    };

    await expect(mutations.authenticate({ assertion })).rejects.toThrow(
      "Invalid or expired authentication challenge",
    );
  });

  it("rejects revoked credentials", async () => {
    process.env.JWT_SECRET = "test-secret";
    const mutations = makeMutationUseCases(
      mockUserService({
        getUserByCredentialId: async () => ({
          userId: "user-1",
          displayName: "Alice",
          status: "active",
          tempoAddress: "0xaaaa111111111111111111111111111111111111",
          username: "",
          publicKey: Buffer.from("stored-key"),
          revoked: true,
        }),
      }),
    );
    const { challenge } = mutations.requestChallenge();
    const assertion = {
      credentialId: "abc",
      authenticatorData: "aa",
      clientDataJSON: Buffer.from(
        JSON.stringify({ challenge, type: "webauthn.get" }),
      ).toString("base64url"),
      signature: "sig",
    };

    await expect(mutations.authenticate({ assertion })).rejects.toThrow(
      "Credential has been revoked",
    );
  });

  it("addCredential requires assertion", async () => {
    process.env.JWT_SECRET = "test-secret";
    const mutations = makeMutationUseCases(mockUserService());
    const { challenge } = mutations.requestChallenge();

    await expect(
      mutations.addCredential({
        newCredential: {
          credentialId: "new",
          clientDataJSON: Buffer.from(
            JSON.stringify({ challenge, type: "webauthn.create" }),
          ).toString("base64url"),
          attestationObject: "att",
        },
        assertion: {
          credentialId: "session",
          authenticatorData: "aa",
          clientDataJSON: Buffer.from(
            JSON.stringify({ challenge: "other", type: "webauthn.get" }),
          ).toString("base64url"),
          signature: "sig",
        },
        publicKey: "00",
        userId: "user-1",
        address: "0xaaaa111111111111111111111111111111111111",
        sessionCredentialId: "session",
      }),
    ).rejects.toThrow("Invalid or expired assertion challenge");
  });
});
