# Build context must be repo root (needs packages/proto/ for grpc-proto-loader at runtime)
FROM oven/bun:1 AS builder
WORKDIR /app

COPY packages/proto/ packages/proto/
COPY services/bff/package.json services/bff/package.json

WORKDIR /app/services/bff
RUN bun install --frozen-lockfile

COPY services/bff/src/ src/
COPY services/bff/tsconfig.json tsconfig.json

FROM oven/bun:1-slim
WORKDIR /app
COPY --from=builder /app/packages/ packages/
COPY --from=builder /app/services/bff/ services/bff/

WORKDIR /app/services/bff
EXPOSE 4000
CMD ["bun", "run", "src/index.ts"]
