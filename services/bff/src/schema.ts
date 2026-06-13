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
  }

  type Mutation {
    """Register a new user with a passkey credential, returns a JWT session"""
    registerUser(
      address: String!
      credentialId: String!
      publicKey: String!
      displayName: String
      chainId: Int
    ): AuthPayload!

    """Add a new passkey credential to the current user's account (requires auth)"""
    addCredential(credentialId: String!, publicKey: String!): String!

    """
    Authenticate an existing user.
    The frontend must have already verified the passkey challenge before calling this.
    Returns a JWT session token.
    """
    authenticate(credentialId: String!, chainId: Int): AuthPayload!

    """Set or update the authenticated user's username (requires auth)"""
    setUsername(username: String!): User!
  }
`;
