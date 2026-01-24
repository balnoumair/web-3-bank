import type { YogaInitialContext } from 'graphql-yoga';
import { sessionService } from '../auth/session-service';
import { userStore } from '../storage/user-store';
import type { User } from '../types';

export interface GraphQLContext {
    user: User | null;
    token: string | null;
}

export function createContext(
    initialContext: YogaInitialContext
): GraphQLContext {
    const authHeader = initialContext.request.headers.get('authorization');
    const token = sessionService.extractTokenFromHeader(authHeader || undefined);

    let user: User | null = null;

    if (token) {
        const payload = sessionService.verifyToken(token);
        if (payload) {
            user = userStore.getUserById(payload.userId) || null;
        }
    }

    return {
        user,
        token,
    };
}
