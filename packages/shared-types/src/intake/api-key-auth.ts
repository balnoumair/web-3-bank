import * as z from "zod";

export const ApiKeyStatusSchema = z.enum(["active", "revoked"]);

export type ApiKeyStatus = z.infer<typeof ApiKeyStatusSchema>;

export const ApiKeyRecordSchema = z.object({
  keyId: z.string(),
  token: z.string(),
  status: ApiKeyStatusSchema,
  createdAt: z.string(),
  revokedAt: z.string().nullable(),
});

export type ApiKeyRecord = z.infer<typeof ApiKeyRecordSchema>;

export interface ApiKeyStore {
  getApiKeyByToken(token: string): Promise<ApiKeyRecord | null>;
}

function toApiKeyRecord(record: ApiKeyRecord): ApiKeyRecord {
  return {
    keyId: record.keyId,
    token: record.token,
    status: record.status,
    createdAt: record.createdAt,
    revokedAt: record.revokedAt,
  };
}

export function extractBearerToken(authorizationHeader: string | undefined): string | null {
  if (!authorizationHeader) {
    return null;
  }

  const normalized = authorizationHeader.trim();
  if (normalized.length === 0) {
    return null;
  }

  const parts = normalized.split(/\s+/);
  if (parts.length !== 2) {
    return null;
  }

  const [scheme, token] = parts;
  if (scheme.toLowerCase() !== "bearer") {
    return null;
  }

  return token.length > 0 ? token : null;
}

export function createApiKeyAuthorizer(args: {
  store: ApiKeyStore;
}): (authorizationHeader: string | undefined) => Promise<boolean> {
  return async (authorizationHeader) => {
    const token = extractBearerToken(authorizationHeader);
    if (!token) {
      return false;
    }

    const record = await args.store.getApiKeyByToken(token);
    return record?.status === "active";
  };
}

export class InMemoryApiKeyStore implements ApiKeyStore {
  private readonly recordsByToken = new Map<string, ApiKeyRecord>();

  constructor(records: ApiKeyRecord[]) {
    for (const record of records) {
      const parsed = ApiKeyRecordSchema.parse(record);
      this.recordsByToken.set(parsed.token, toApiKeyRecord(parsed));
    }
  }

  async getApiKeyByToken(token: string): Promise<ApiKeyRecord | null> {
    const record = this.recordsByToken.get(token);
    return record ? toApiKeyRecord(record) : null;
  }

  rotateKey(args: {
    previousKeyId: string;
    nextKeyId: string;
    nextToken: string;
    rotatedAt: string;
  }): void {
    this.revokeKey({
      keyId: args.previousKeyId,
      revokedAt: args.rotatedAt,
    });

    this.recordsByToken.set(args.nextToken, {
      keyId: args.nextKeyId,
      token: args.nextToken,
      status: "active",
      createdAt: args.rotatedAt,
      revokedAt: null,
    });
  }

  revokeKey(args: { keyId: string; revokedAt: string }): void {
    for (const [token, record] of this.recordsByToken.entries()) {
      if (record.keyId === args.keyId) {
        this.recordsByToken.set(token, {
          ...record,
          status: "revoked",
          revokedAt: args.revokedAt,
        });
      }
    }
  }
}
