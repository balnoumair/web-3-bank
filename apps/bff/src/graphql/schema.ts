import { createSchema } from 'graphql-yoga';
import { resolvers } from './resolvers';

export const typeDefs = /* GraphQL */ `
  scalar JSON

  type User {
    id: ID!
    username: String!
    displayName: String!
    createdAt: String!
  }

  type AuthSession {
    token: String!
    user: User!
  }

  type RegistrationOptions {
    options: JSON!
  }

  type AuthenticationOptions {
    options: JSON!
  }

  type Query {
    """
    Get the currently authenticated user
    """
    me: User

    """
    Check if the current session is valid
    """
    checkSession: Boolean!
  }

  type Mutation {
    """
    Start the passkey registration flow
    Returns WebAuthn registration options
    """
    startRegistration(username: String!, displayName: String!): RegistrationOptions!

    """
    Complete the passkey registration flow
    Verifies the credential and creates a new user session
    """
    completeRegistration(credential: JSON!): AuthSession!

    """
    Start the passkey authentication flow
    Returns WebAuthn authentication options
    Username is optional for usernameless flow
    """
    startAuthentication(username: String): AuthenticationOptions!

    """
    Complete the passkey authentication flow
    Verifies the assertion and creates a user session
    """
    completeAuthentication(credential: JSON!): AuthSession!

    """
    Logout the current user
    """
    logout: Boolean!
  }
`;

export const schema = createSchema({
    typeDefs,
    resolvers,
});
