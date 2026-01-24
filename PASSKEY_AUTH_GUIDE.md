# Passkey Authentication - Getting Started

This guide will help you run the passkey authentication system we just implemented.

## Prerequisites

1. **Bun** (for running the BFF service)
   ```bash
   curl -fsSL https://bun.sh/install | bash
   ```

2. **pnpm** (already configured in this repo)

3. **HTTPS** (required for WebAuthn in production, but localhost works for development)

## Installation

Dependencies are already installed via `pnpm install` at the root level.

## Running the Services

You need to run **two services** in separate terminals:

### Terminal 1: BFF Service (Backend for Frontend)

```bash
cd apps/bff
bun run dev
```

The BFF will start on `http://localhost:4000`
- GraphQL endpoint: `http://localhost:4000/graphql`
- Health check: `http://localhost:4000/health`

### Terminal 2: Frontend (Bank Client)

```bash
cd apps/bank-client
pnpm run dev
```

The frontend will start on `http://localhost:3000`

## Testing the Authentication Flow

### 1. Create an Account (Registration)

1. Navigate to `http://localhost:3000/register`
2. Enter a username (e.g., "alice")
3. Enter a display name (e.g., "Alice Smith")
4. Click "Create Passkey"
5. Your browser will prompt you to create a passkey using:
   - Touch ID (Mac)
   - Face ID (iPhone/iPad)
   - Windows Hello (Windows)
   - Fingerprint sensor (Android)
6. After successful creation, you'll be redirected to the dashboard

### 2. Login

1. Logout from the dashboard (hover over your profile picture, click "Logout")
2. Navigate to `http://localhost:3000/login`
3. Click "Sign in with Passkey" for usernameless flow, OR
4. Enter your username and click "Continue"
5. Authenticate with your biometric
6. You'll be redirected to the dashboard

### 3. Explore the Dashboard

- View your portfolio (mock data for now)
- See recent activity
- Notice your authenticated user info in the top-right
- Try refreshing the page - you should stay logged in (session persistence)

## Architecture

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   Browser   │────────▶│     BFF     │────────▶│   Backend   │
│  (SolidJS)  │  GraphQL│ (Bun+Yoga)  │   gRPC  │   (Rust)    │
│             │◀────────│             │◀────────│             │
└─────────────┘         └─────────────┘         └─────────────┘
      │                       │                       │
      │ WebAuthn              │ SimpleWebAuthn        │ Identity
      │ (Passkeys)            │ JWT Sessions          │ Service
      │                       │ In-Memory Store       │ (Phase 2)
      ▼                       ▼                       ▼
```

### Current Implementation (Phase 1)

- ✅ Frontend: SolidJS with WebAuthn
- ✅ BFF: Bun + Express + GraphQL Yoga
- ✅ Authentication: SimpleWebAuthn library
- ✅ Session: JWT tokens (7-day expiry)
- ✅ Storage: In-memory (BFF)

### Future (Phase 2)

- ⏳ Backend: Rust + gRPC Identity service
- ⏳ Storage: Persistent database with event sourcing
- ⏳ On-chain: Account Abstraction with passkey signing

## Security Notes

### What Sessions Grant Access To

✅ **Allowed:**
- View portfolio balance
- View transaction history
- View account settings
- Request payment links

❌ **NOT Allowed:**
- Move funds
- Change on-chain policies
- Execute transactions

**Important:** Login sessions do NOT grant fund-movement authority. That will require separate on-chain authorization in Phase 2.

## Troubleshooting

### "Command not found: bun"

Install Bun:
```bash
curl -fsSL https://bun.sh/install | bash
```

### "Cannot connect to BFF"

Make sure the BFF service is running on port 4000:
```bash
cd apps/bff
bun run dev
```

### "Passkey creation failed"

- Make sure you're on `localhost` (not 127.0.0.1)
- Check that your browser supports WebAuthn (Chrome, Safari, Firefox, Edge all do)
- Ensure you have a biometric sensor or security key available

### "Session not persisting"

- Check browser console for errors
- Verify localStorage has `auth_token`
- Make sure cookies are enabled

## GraphQL Playground

You can explore the GraphQL API at `http://localhost:4000/graphql`

Example queries:

```graphql
# Check if logged in
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

# Start registration
mutation {
  startRegistration(username: "bob", displayName: "Bob Jones") {
    options
  }
}
```

## Next Steps

1. ✅ Test registration and login flows
2. ✅ Verify session persistence
3. ⏳ Implement Rust backend Identity service
4. ⏳ Add Account Abstraction integration
5. ⏳ Implement on-chain authorization flows

## Questions?

Refer to:
- Implementation plan: `.gemini/antigravity/brain/.../implementation_plan.md`
- BFF README: `apps/bff/README.md`
- Architecture docs: `docs/`
