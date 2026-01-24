import { GraphQLClient } from 'graphql-request';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:4000/graphql';

class GraphQLService {
    private client: GraphQLClient;
    private token: string | null = null;

    constructor() {
        this.client = new GraphQLClient(API_URL);
        // Load token from localStorage on initialization
        if (typeof window !== 'undefined') {
            this.token = localStorage.getItem('auth_token');
            if (this.token) {
                this.setAuthToken(this.token);
            }
        }
    }

    setAuthToken(token: string | null) {
        this.token = token;
        if (token) {
            this.client.setHeader('authorization', `Bearer ${token}`);
            if (typeof window !== 'undefined') {
                localStorage.setItem('auth_token', token);
            }
        } else {
            this.client.setHeader('authorization', '');
            if (typeof window !== 'undefined') {
                localStorage.removeItem('auth_token');
            }
        }
    }

    getToken(): string | null {
        return this.token;
    }

    async request<T>(query: string, variables?: Record<string, any>): Promise<T> {
        return this.client.request<T>(query, variables);
    }
}

export const graphqlService = new GraphQLService();
