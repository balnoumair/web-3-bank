export interface User {
    id: string;
    username: string;
    displayName: string;
    createdAt: Date;
}

export interface PasskeyCredential {
    credentialId: string;
    publicKey: Uint8Array;
    counter: number;
    userId: string;
    createdAt: Date;
    transports?: AuthenticatorTransport[];
}

export interface AuthSession {
    token: string;
    user: User;
}

export interface Challenge {
    challenge: string;
    userId?: string;
    createdAt: Date;
}
