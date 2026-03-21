import { createContext, useContext, type ParentComponent } from 'solid-js';
import { createAuth, type AuthValue } from '~/hooks/use-auth';

const AuthContext = createContext<AuthValue>();

export const AuthProvider: ParentComponent = (props) => {
  const auth = createAuth();
  return (
    <AuthContext.Provider value={auth}>{props.children}</AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};
