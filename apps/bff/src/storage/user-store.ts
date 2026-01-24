import type { User, PasskeyCredential, Challenge } from '../types';

/**
 * In-memory storage for Phase 1
 * TODO: Replace with backend Identity service in Phase 2
 */
class UserStore {
    private users: Map<string, User> = new Map();
    private credentials: Map<string, PasskeyCredential> = new Map();
    private challenges: Map<string, Challenge> = new Map();
    private usersByUsername: Map<string, string> = new Map(); // username -> userId

    // User operations
    createUser(username: string, displayName: string): User {
        const id = crypto.randomUUID();
        const user: User = {
            id,
            username,
            displayName,
            createdAt: new Date(),
        };
        this.users.set(id, user);
        this.usersByUsername.set(username, id);
        return user;
    }

    getUserById(id: string): User | undefined {
        return this.users.get(id);
    }

    getUserByUsername(username: string): User | undefined {
        const userId = this.usersByUsername.get(username);
        return userId ? this.users.get(userId) : undefined;
    }

    usernameExists(username: string): boolean {
        return this.usersByUsername.has(username);
    }

    // Credential operations
    storeCredential(credential: PasskeyCredential): void {
        this.credentials.set(credential.credentialId, credential);
    }

    getCredential(credentialId: string): PasskeyCredential | undefined {
        return this.credentials.get(credentialId);
    }

    getCredentialsByUserId(userId: string): PasskeyCredential[] {
        return Array.from(this.credentials.values()).filter(
            (cred) => cred.userId === userId
        );
    }

    updateCredentialCounter(credentialId: string, counter: number): void {
        const credential = this.credentials.get(credentialId);
        if (credential) {
            credential.counter = counter;
        }
    }

    // Challenge operations
    storeChallenge(challenge: string, userId?: string): void {
        this.challenges.set(challenge, {
            challenge,
            userId,
            createdAt: new Date(),
        });

        // Clean up old challenges (older than 5 minutes)
        const fiveMinutesAgo = Date.now() - 5 * 60 * 1000;
        for (const [key, value] of this.challenges.entries()) {
            if (value.createdAt.getTime() < fiveMinutesAgo) {
                this.challenges.delete(key);
            }
        }
    }

    getChallenge(challenge: string): Challenge | undefined {
        return this.challenges.get(challenge);
    }

    deleteChallenge(challenge: string): void {
        this.challenges.delete(challenge);
    }
}

export const userStore = new UserStore();
