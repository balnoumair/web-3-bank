/** GraphQL SDL — type definitions only. Resolvers are wired in index.ts. */
export const typeDefs = /* GraphQL */ `
  type User {
    userId: String!
    displayName: String!
    status: String!
    tempoAddress: String!
    username: String!
    """Hot-path destination chain (EIP-155 id). Only set on send-preview queries."""
    destChainId: String
  }

  type RecipientRouting {
    tempoAddress: String!
    destChainId: String!
  }

  type WithdrawalRoutingEntry {
    chainId: String!
    withdrawableWei: String!
    available: Boolean!
    reason: String
    balanceWei: String!
  }

  type Credential {
    credentialId: String!
    tempoAddress: String!
    createdAt: String!
    revoked: Boolean!
  }

  type PoolDepth {
    chainId: String!
    depthWei: String!
  }

  type Balance {
    amountWei: String!
    """True when one or more chains used indexed fallback instead of live balanceOf."""
    degraded: Boolean
  }

  type Transfer {
    id: String!
    from: String!
    to: String!
    amount: String!
    timestamp: String!
    txHash: String!
    kind: String
    direction: String
  }

  type AuthPayload {
    token: String!
    userId: String!
  }

  type AuthChallenge {
    challenge: String!
  }

  input WebAuthnAssertionInput {
    credentialId: String!
    authenticatorData: String!
    clientDataJSON: String!
    signature: String!
  }

  input WebAuthnAttestationInput {
    credentialId: String!
    clientDataJSON: String!
    attestationObject: String!
  }

  type Query {
    """Returns current user profile from JWT session (requires auth)"""
    me: User!

    """Returns the authenticated user's aggregated SyncUSD balance (requires auth)"""
    balance: Balance!

    """Returns pool depth in wei for a given chain ID"""
    poolDepths(chainId: Int!): PoolDepth!

    """Returns recent transfer history for the authenticated user (requires auth)"""
    recentTransfers(limit: Int): [Transfer!]!

    """Resolve a username to a user profile — used when sending funds (requires auth)"""
    resolveUsername(username: String!): User!

    """Resolve a raw Tempo address to hot-path routing (requires auth)"""
    resolveRecipientRouting(tempoAddress: String!): RecipientRouting!

    """Per-chain withdrawal routing for the authenticated user (requires auth)"""
    withdrawalRouting: [WithdrawalRoutingEntry!]!

    """Passkeys registered to the authenticated account (requires auth)"""
    credentials: [Credential!]!
  }

  type Mutation {
    """Issue a single-use WebAuthn challenge (anonymous)"""
    requestChallenge: AuthChallenge!

    """Register a new user with a verified passkey attestation"""
    registerUser(
      attestation: WebAuthnAttestationInput!
      address: String!
      publicKey: String!
      displayName: String
      chainId: Int
    ): AuthPayload!

    """Add a new passkey credential (requires auth + fresh assertion)"""
    addCredential(
      newCredential: WebAuthnAttestationInput!
      assertion: WebAuthnAssertionInput!
      publicKey: String!
    ): String!

    """Authenticate with a verified WebAuthn assertion"""
    authenticate(
      assertion: WebAuthnAssertionInput!
      chainId: Int
    ): AuthPayload!

    """Dev-only legacy login by credentialId (requires BFF_DEV_MODE=1)"""
    authenticateLegacy(credentialId: String!, chainId: Int): AuthPayload!

    """Set or update the authenticated user's username (requires auth)"""
    setUsername(username: String!): User!
  }
`;
