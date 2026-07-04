import { gql } from '~/lib/graphql';
import {
  REQUEST_CHALLENGE_MUTATION,
  type RequestChallengeResponse,
} from '~/queries/auth';

export async function requestServerChallenge(): Promise<string> {
  const data = await gql<RequestChallengeResponse>(REQUEST_CHALLENGE_MUTATION);
  return data.requestChallenge.challenge;
}
