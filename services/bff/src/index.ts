import { createYoga } from "graphql-yoga";
import { schema } from "./schema.js";
import { buildContext } from "./context.js";

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
