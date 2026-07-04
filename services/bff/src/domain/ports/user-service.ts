export type UserRecord = {
  userId: string;
  displayName: string;
  status: string;
  tempoAddress: string;
  username: string;  // empty string if not set
  /** Populated only by send-routing queries (e.g. `resolveUsername`). */
  destChainId?: string | null;
};

export type CredentialAuthRecord = UserRecord & {
  publicKey: Buffer;
  revoked: boolean;
};

export type CredentialRecord = {
  credentialId: string;
  tempoAddress: string;
  createdAt: string;
  revoked: boolean;
};

export type CreateUserInput = {
  displayName?: string;
  credentialId: Buffer;
  publicKey: Buffer;
  tempoAddress: string;
};

export type AddCredentialInput = {
  userId: string;
  credentialId: Buffer;
  publicKey: Buffer;
  tempoAddress: string;
};

export type HomeChainResult =
  | { found: true; chainId: string }
  | { found: false };

/** Driven port — implemented by the gRPC user-service adapter. */
export interface IUserService {
  createUser(input: CreateUserInput): Promise<{ userId: string }>;
  getUserByAddress(tempoAddress: string): Promise<UserRecord>;
  getUserByCredentialId(credentialId: Buffer): Promise<CredentialAuthRecord>;
  addCredential(input: AddCredentialInput): Promise<{ credentialId: string }>;
  setUsername(userId: string, username: string): Promise<UserRecord>;
  getUserByUsername(username: string): Promise<UserRecord>;
  getUserHomeChain(tempoAddress: string): Promise<HomeChainResult>;
  listCredentials(userId: string): Promise<CredentialRecord[]>;
}
