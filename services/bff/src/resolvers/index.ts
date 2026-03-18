import { queryResolvers } from "./query.js";
import { mutationResolvers } from "./mutation.js";

export const resolvers = {
  Query: queryResolvers,
  Mutation: mutationResolvers,
};
