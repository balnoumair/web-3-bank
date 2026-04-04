import { Title } from "@solidjs/meta";
import { createSignal, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "~/contexts/auth-context";

export default function Login() {
  const navigate = useNavigate();
  const auth = useAuth();
  const [isSubmitting, setIsSubmitting] = createSignal(false);

  const handlePasskeyLogin = async () => {
    setIsSubmitting(true);
    try {
      await auth.login();
      navigate("/");
    } catch (error) {
      console.error("Login error:", error);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <main class="min-h-screen bg-bg flex items-center justify-center p-4 relative overflow-hidden">
      <Title>Sign In - Web3Bank</Title>

      {/* Ambient glow */}
      <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] bg-accent/[0.04] rounded-full blur-[120px] pointer-events-none" />

      <div class="w-full max-w-sm animate-in relative">
        {/* Logo */}
        <div class="text-center mb-10">
          <div class="inline-flex items-center justify-center w-14 h-14 bg-accent/10 rounded-2xl mb-5 ring-1 ring-accent/20">
            <span class="text-accent font-bold text-xl font-[Satoshi]">W3</span>
          </div>
          <h1 class="text-3xl font-bold text-text font-[Satoshi] tracking-tight">
            Web3Bank
          </h1>
          <p class="text-muted text-sm mt-2">
            Stablecoin banking, simplified.
          </p>
        </div>

        {/* Card */}
        <div class="bg-surface border border-edge rounded-2xl p-8">
          <h2 class="text-xl font-bold text-text font-[Satoshi] mb-1">
            Welcome back
          </h2>
          <p class="text-muted text-sm mb-8">
            Sign in with your passkey to continue.
          </p>

          {/* Error */}
          <Show when={auth.error()}>
            <div class="mb-6 p-3 bg-error/10 border border-error/20 rounded-xl">
              <p class="text-error text-sm">{auth.error()}</p>
            </div>
          </Show>

          {/* Passkey CTA */}
          <button
            onClick={handlePasskeyLogin}
            disabled={isSubmitting()}
            class="w-full bg-accent hover:bg-accent-hover active:scale-[0.98] text-accent-fg font-semibold py-3.5 px-4 rounded-xl transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2.5"
          >
            <Show
              when={!isSubmitting()}
              fallback={
                <span class="flex items-center gap-2">
                  <svg class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
                    <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="60" stroke-linecap="round" />
                  </svg>
                  Authenticating...
                </span>
              }
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
              </svg>
              Sign in with Passkey
            </Show>
          </button>

          {/* Register link */}
          <div class="mt-6 text-center">
            <p class="text-subtle text-sm">
              Don't have an account?{" "}
              <a
                href="/register"
                class="text-accent hover:text-accent-hover font-medium transition-colors"
              >
                Create one
              </a>
            </p>
          </div>
        </div>

        {/* Security note */}
        <p class="text-subtle text-xs text-center mt-6">
          Secured with device passkeys. Your biometric data never leaves your device.
        </p>
      </div>
    </main>
  );
}
