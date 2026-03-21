import { MetaProvider, Title } from "@solidjs/meta";
import { Router } from "@solidjs/router";
import { FileRoutes } from "@solidjs/start/router";
import { Suspense } from "solid-js";
import { QueryProvider } from "~/contexts/query-context";
import { AuthProvider } from "~/contexts/auth-context";
import BankLayout from "~/components/BankLayout";
import "./app.css";

export default function App() {
  return (
    <QueryProvider>
      <AuthProvider>
        <Router
          root={(props) => (
            <MetaProvider>
              <Title>Web3Bank</Title>
              <BankLayout>
                <Suspense>{props.children}</Suspense>
              </BankLayout>
            </MetaProvider>
          )}
        >
          <FileRoutes />
        </Router>
      </AuthProvider>
    </QueryProvider>
  );
}
