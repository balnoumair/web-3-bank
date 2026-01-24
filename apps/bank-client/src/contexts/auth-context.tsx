import {
    createContext,
    useContext,
    createSignal,
    createEffect,
    onMount,
    type ParentComponent,
} from 'solid-js';
import { passkeyService, type User, type AuthSession } from '~/services/passkey-service';
import { graphqlService } from '~/services/graphql-client';

const ME_QUERY = `
  query Me {
    me {
      id
      username
      displayName
      createdAt
    }
  }
`;

interface AuthContextValue {
    user: () => User | null;
    isAuthenticated: () => boolean;
    isLoading: () => boolean;
    register: (username: string, displayName: string) => Promise<void>;
    login: (username?: string) => Promise<void>;
    logout: () => void;
    error: () => string | null;
}

const AuthContext = createContext<AuthContextValue>();

export const AuthProvider: ParentComponent = (props) => {
    const [user, setUser] = createSignal<User | null>(null);
    const [isLoading, setIsLoading] = createSignal(true);
    const [error, setError] = createSignal<string | null>(null);

    // Check for existing session on mount
    onMount(async () => {
        const token = graphqlService.getToken();
        if (token) {
            try {
                const { me } = await graphqlService.request<{ me: User | null }>(ME_QUERY);
                if (me) {
                    setUser(me);
                } else {
                    // Token is invalid, clear it
                    graphqlService.setAuthToken(null);
                }
            } catch (err) {
                console.error('Failed to fetch user:', err);
                // Token is invalid, clear it
                graphqlService.setAuthToken(null);
            }
        }
        setIsLoading(false);
    });

    const register = async (username: string, displayName: string) => {
        setIsLoading(true);
        setError(null);
        try {
            const session = await passkeyService.register(username, displayName);
            setUser(session.user);
        } catch (err) {
            const message = err instanceof Error ? err.message : 'Registration failed';
            setError(message);
            throw err;
        } finally {
            setIsLoading(false);
        }
    };

    const login = async (username?: string) => {
        setIsLoading(true);
        setError(null);
        try {
            const session = await passkeyService.authenticate(username);
            setUser(session.user);
        } catch (err) {
            const message = err instanceof Error ? err.message : 'Login failed';
            setError(message);
            throw err;
        } finally {
            setIsLoading(false);
        }
    };

    const logout = () => {
        passkeyService.logout();
        setUser(null);
        setError(null);
    };

    const value: AuthContextValue = {
        user,
        isAuthenticated: () => user() !== null,
        isLoading,
        register,
        login,
        logout,
        error,
    };

    return <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>;
};

export const useAuth = () => {
    const context = useContext(AuthContext);
    if (!context) {
        throw new Error('useAuth must be used within an AuthProvider');
    }
    return context;
};
