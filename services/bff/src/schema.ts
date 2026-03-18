import { createSchema } from "graphql-yoga";
import { resolvers } from "./resolvers/index.js";

export const schema = createSchema({
  typeDefs: /* GraphQL */ `
    type User {
      userId: String!
      displayName: String!
      status: String!
      tempoAddress: String!
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

    type Transfer {
      id: String!
      from: String!
      to: String!
      amount: String!
      timestamp: String!
      txHash: String!
    }

    type AuthPayload {
      token: String!
      userId: String!
    }

    type Query {
      """Returns current user profile from JWT session (requires auth)"""
      me: User!

      """Returns the authenticated user's SyncUSD balance (requires auth)"""
      balance: String!

      """Returns pool depth in wei for a given chain ID"""
      poolDepths(chainId: Int!): PoolDepth!

      """Returns recent transfer history for the authenticated user (requires auth)"""
      recentTransfers(limit: Int): [Transfer!]!
    }

    type Mutation {
      """Register a new user with a passkey credential, returns a JWT session"""
      registerUser(
        address: String!
        credentialId: String!
        publicKey: String!
        displayName: String
      ): AuthPayload!

      """Add a new passkey credential to the current user's account (requires auth)"""
      addCredential(credentialId: String!, publicKey: String!): String!

      """
      Authenticate an existing user.
      The frontend must have already verified the passkey challenge before calling this.
      Returns a JWT session token.
      """
      authenticate(address: String!, credentialId: String!): AuthPayload!
    }
  `,
  resolvers,
});
