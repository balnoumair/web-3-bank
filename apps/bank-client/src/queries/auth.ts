export interface User {
  userId: string;
  displayName: string;
  status: string;
  tempoAddress: string;
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
  mutation Authenticate($address: String!, $credentialId: String!) {
    authenticate(address: $address, credentialId: $credentialId) {
      token
      userId
    }
  }
`;

export interface AuthenticateResponse {
  authenticate: AuthPayload;
}
