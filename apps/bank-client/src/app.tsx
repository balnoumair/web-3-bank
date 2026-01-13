import { MetaProvider, Title } from "@solidjs/meta";
import { Router } from "@solidjs/router";
import { FileRoutes } from "@solidjs/start/router";
import { Suspense } from "solid-js";
import BankLayout from "~/components/BankLayout";
import "./app.css";

export default function App() {
  return (
    <Router
      root={props => (
        <MetaProvider>
          <Title>Web3Bank - Simplified Stablecoin Banking</Title>
          <BankLayout>
            <Suspense>{props.children}</Suspense>
          </BankLayout>
        </MetaProvider>
      )}
    >
      <FileRoutes />
    </Router>
  );
}
