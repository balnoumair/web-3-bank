# BFF (Backend for Frontend) Service

GraphQL API service for the Web3Bank application, handling authentication and session management.

## Overview

This BFF service provides:
- Passkey-based authentication (WebAuthn)
- Session management with JWT tokens
- GraphQL API for frontend communication
- User and credential storage (Phase 1: in-memory)

## Tech Stack

- **Runtime**: Bun
- **Framework**: Express
- **GraphQL**: GraphQL Yoga
- **Authentication**: SimpleWebAuthn + JWT

## Getting Started

### Prerequisites

- Bun >= 1.0.0
- Node.js >= 22 (for compatibility)

### Installation

```bash
# Install dependencies
bun install

# Copy environment template
cp .env.example .env

# Edit .env with your configuration
```

### Development

```bash
# Start development server with hot reload
bun run dev
```

The server will start on `http://localhost:4000` with GraphQL endpoint at `/graphql`.

### Production

```bash
# Start production server
bun run start
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `4000` |
| `JWT_SECRET` | Secret key for JWT signing | (required) |
| `FRONTEND_URL` | Frontend origin for CORS | `http://localhost:3000` |
| `RP_NAME` | WebAuthn Relying Party name | `Web3Bank` |
| `RP_ID` | WebAuthn Relying Party ID | `localhost` |
| `RP_ORIGIN` | WebAuthn expected origin | `http://localhost:3000` |

## API Documentation

### GraphQL Schema

See [src/graphql/schema.ts](src/graphql/schema.ts) for the complete schema.

### Authentication Flow

#### Registration

1. Call `startRegistration(username, displayName)` mutation
2. Use returned options with WebAuthn API on client
3. Call `completeRegistration(credential)` mutation
4. Receive session token and user data

#### Login

1. Call `startAuthentication(username?)` mutation
2. Use returned options with WebAuthn API on client
3. Call `completeAuthentication(credential)` mutation
4. Receive session token and user data

### Example Queries

```graphql
# Check current session
query {
  checkSession
}

# Get current user
query {
  me {
    id
    username
    displayName
  }
}
```

### Example Mutations

```graphql
# Start registration
mutation {
  startRegistration(username: "alice", displayName: "Alice Smith") {
    options
  }
}

# Complete registration
mutation {
  completeRegistration(credential: $credential) {
    token
    user {
      id
      username
      displayName
    }
  }
}
```

## Architecture Notes

### Phase 1 (Current)

- In-memory storage for users and credentials
- BFF handles all authentication logic
- Suitable for development and prototyping

### Phase 2 (Future)

- Backend Identity service (Rust + gRPC)
- BFF becomes pure orchestration layer
- Persistent storage with event sourcing

## Security Considerations

- **HTTPS Required**: WebAuthn requires HTTPS in production
- **JWT Secret**: Use a strong, random secret in production
- **Challenge Expiry**: Challenges expire after 5 minutes
- **Counter Validation**: Credential counters prevent replay attacks
- **Session Scope**: Sessions grant view-only access, NOT fund movement authority

## Testing

```bash
# Run tests
bun test
```

## License

Private - Web3Bank Project
