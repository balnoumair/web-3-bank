import jwt from 'jsonwebtoken';
import type { User, AuthSession } from '../types';

const JWT_SECRET = process.env.JWT_SECRET || 'your-secret-key-change-in-production';
const JWT_EXPIRES_IN = '7d';

export interface TokenPayload {
    userId: string;
    username: string;
    iat?: number;
    exp?: number;
}

export class SessionService {
    /**
     * Create a new session token for a user
     */
    createSession(user: User): AuthSession {
        const payload: TokenPayload = {
            userId: user.id,
            username: user.username,
        };

        const token = jwt.sign(payload, JWT_SECRET, {
            expiresIn: JWT_EXPIRES_IN,
        });

        return {
            token,
            user,
        };
    }

    /**
     * Verify and decode a session token
     */
    verifyToken(token: string): TokenPayload | null {
        try {
            const decoded = jwt.verify(token, JWT_SECRET) as TokenPayload;
            return decoded;
        } catch (error) {
            console.error('Token verification failed:', error);
            return null;
        }
    }

    /**
     * Extract token from Authorization header
     */
    extractTokenFromHeader(authHeader?: string): string | null {
        if (!authHeader) {
            return null;
        }

        const parts = authHeader.split(' ');
        if (parts.length !== 2 || parts[0] !== 'Bearer') {
            return null;
        }

        return parts[1];
    }
}

export const sessionService = new SessionService();
