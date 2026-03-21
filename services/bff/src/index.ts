import { createSchema, createYoga } from "graphql-yoga";
import { typeDefs } from "./schema.js";
import { buildContext } from "./context.js";
import { makeQueryResolvers, makeMutationResolvers } from "./resolvers/index.js";
import { makeQueryUseCases } from "./application/queries.js";
import { makeMutationUseCases } from "./application/mutations.js";
import { GrpcUserServiceAdapter } from "./infrastructure/grpc/user-service.adapter.js";
import { HttpTreasuryServiceAdapter } from "./infrastructure/http/treasury-service.adapter.js";

// ── Composition root ──────────────────────────────────────────────────────────
// Adapters (infrastructure)
const userService = new GrpcUserServiceAdapter();
const treasuryService = new HttpTreasuryServiceAdapter();

// Use cases (application)
const queries = makeQueryUseCases(userService, treasuryService);
const mutations = makeMutationUseCases(userService);

// Schema (presentation)
const schema = createSchema({
  typeDefs,
  resolvers: {
    Query: makeQueryResolvers(queries),
    Mutation: makeMutationResolvers(mutations),
  },
});

// ── Server ────────────────────────────────────────────────────────────────────
const yoga = createYoga({
  schema,
  context: buildContext,
  graphiql: true,
  logging: true,
});

const port = process.env.PORT ? parseInt(process.env.PORT) : 4000;

const server = Bun.serve({
  port,
  fetch: yoga.fetch,
});

console.log(
  `BFF GraphQL server running at http://localhost:${server.port}/graphql`
);
