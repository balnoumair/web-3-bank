export interface User {
  userId: string;
  displayName: string;
  status: string;
  tempoAddress: string;
  username: string;
}

export interface AuthPayload {
  token: string;
  userId: string;
}

export interface AuthChallenge {
  challenge: string;
}

export const ME_QUERY = `
  query Me {
    me {
      userId
      displayName
      status
      tempoAddress
      username
    }
  }
`;

export interface MeResponse {
  me: User;
}

export const REQUEST_CHALLENGE_MUTATION = `
  mutation RequestChallenge {
    requestChallenge {
      challenge
    }
  }
`;

export interface RequestChallengeResponse {
  requestChallenge: AuthChallenge;
}

export const REGISTER_USER_MUTATION = `
  mutation RegisterUser(
    $attestation: WebAuthnAttestationInput!
    $address: String!
    $publicKey: String!
    $displayName: String
    $chainId: Int
  ) {
    registerUser(
      attestation: $attestation
      address: $address
      publicKey: $publicKey
      displayName: $displayName
      chainId: $chainId
    ) {
      token
      userId
    }
  }
`;

export interface RegisterUserResponse {
  registerUser: AuthPayload;
}

export const AUTHENTICATE_MUTATION = `
  mutation Authenticate($assertion: WebAuthnAssertionInput!, $chainId: Int) {
    authenticate(assertion: $assertion, chainId: $chainId) {
      token
      userId
    }
  }
`;

export interface AuthenticateResponse {
  authenticate: AuthPayload;
}

export interface Credential {
  credentialId: string;
  tempoAddress: string;
  createdAt: string;
  revoked: boolean;
}

export const CREDENTIALS_QUERY = `
  query Credentials {
    credentials {
      credentialId
      tempoAddress
      createdAt
      revoked
    }
  }
`;

export interface CredentialsResponse {
  credentials: Credential[];
}

export const ADD_CREDENTIAL_MUTATION = `
  mutation AddCredential(
    $newCredential: WebAuthnAttestationInput!
    $assertion: WebAuthnAssertionInput!
    $publicKey: String!
  ) {
    addCredential(
      newCredential: $newCredential
      assertion: $assertion
      publicKey: $publicKey
    )
  }
`;

export interface AddCredentialResponse {
  addCredential: string;
}

export const SET_USERNAME_MUTATION = `
  mutation SetUsername($username: String!) {
    setUsername(username: $username) {
      userId
      displayName
      status
      tempoAddress
      username
    }
  }
`;

export interface SetUsernameResponse {
  setUsername: User;
}

export const RESOLVE_USERNAME_QUERY = `
  query ResolveUsername($username: String!) {
    resolveUsername(username: $username) {
      userId
      tempoAddress
      username
      destChainId
    }
  }
`;

export interface ResolveUsernameResponse {
  resolveUsername: User & { destChainId?: string | null };
}

export const RESOLVE_RECIPIENT_ROUTING_QUERY = `
  query ResolveRecipientRouting($tempoAddress: String!) {
    resolveRecipientRouting(tempoAddress: $tempoAddress) {
      tempoAddress
      destChainId
    }
  }
`;

export interface ResolveRecipientRoutingResponse {
  resolveRecipientRouting: { tempoAddress: string; destChainId: string };
}
