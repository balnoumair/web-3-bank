# Claude Instructions for web3Bank

## Working Style

This is a **learning-oriented project**. The developer is not a web3 expert and wants to understand what is being done and why at every step — not just see code produced. Is not familiar also with Rust programing language

### How to explain things

- Go **step by step**. Never jump ahead without explaining what the next step is and why.
- Use **plain-language analogies** for web3 concepts (testnets, gas, private keys, proxies, RPC, etc.).
- When something surprising or non-obvious comes up (e.g. a chain having USD instead of ETH as gas), **pause and explain it** before moving on.
- Keep a **running progress table** for multi-step tasks so the developer always knows where we are.
- When a decision is made (e.g. deploying ERC-20 instead of TIP-20), **explain the tradeoff** and why it's acceptable.
- Flag mistakes clearly and explain **why** something went wrong (e.g. wrong address in env file) rather than just giving the fix.

### Assumptions

- Do not assume familiarity with: private keys vs public addresses, testnets vs mainnet, gas tokens, contract proxies, RPC URLs, chain IDs, or any other web3 primitive.
- Do not assume Rust programing language knowledge
- Do assume familiarity with: general software development, running terminal commands, editing files, git.

### Security

- **NEVER read or edit `.env` files.** They contain private keys and secrets. Only reference `.env.example` templates.
- When env changes are needed, tell the user what to set — do not read or modify the file directly.

### Architecture

- **Database isolation:** each service owns its own PostgreSQL schema and must never query another service's schema.
  - `user-service` → `users.*` only
  - `treasury` → `treasury.*` only
  - No cross-schema joins or queries, ever. If data from another service is needed, go through its gRPC API.
  - `search_path` is enforced in code via `after_connect` in each service's pool setup.
  - Role-based enforcement (DB-level permissions) is a TODO — currently blocked by Supabase pooler not supporting custom roles.

### Tone

- Instructive but not condescending.
- Keep responses concise — explain concepts inline, don't write essays.
