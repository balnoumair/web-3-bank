export type UserRecord = {
  userId: string;
  displayName: string;
  status: string;
  tempoAddress: string;
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

/** Driven port — implemented by the gRPC user-service adapter. */
export interface IUserService {
  createUser(input: CreateUserInput): Promise<{ userId: string }>;
  getUserByAddress(tempoAddress: string): Promise<UserRecord>;
  getUserByCredentialId(credentialId: Buffer): Promise<UserRecord>;
  addCredential(input: AddCredentialInput): Promise<{ credentialId: string }>;
}
