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

export const REGISTER_USER_MUTATION = `
  mutation RegisterUser(
    $address: String!
    $credentialId: String!
    $publicKey: String!
    $displayName: String
  ) {
    registerUser(
      address: $address
      credentialId: $credentialId
      publicKey: $publicKey
      displayName: $displayName
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
  mutation Authenticate($credentialId: String!) {
    authenticate(credentialId: $credentialId) {
      token
      userId
    }
  }
`;

export interface AuthenticateResponse {
  authenticate: AuthPayload;
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
    }
  }
`;

export interface ResolveUsernameResponse {
  resolveUsername: User;
}
