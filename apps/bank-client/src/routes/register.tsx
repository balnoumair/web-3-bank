import { Title } from "@solidjs/meta";
import { createSignal, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "~/contexts/auth-context";

export default function Register() {
  const navigate = useNavigate();
  const auth = useAuth();
  const [displayName, setDisplayName] = createSignal("");
  const [isSubmitting, setIsSubmitting] = createSignal(false);

  const handleRegister = async (e: Event) => {
    e.preventDefault();
    if (!displayName()) return;

    setIsSubmitting(true);
    try {
      await auth.register(displayName());
      navigate("/");
    } catch (error) {
      console.error("Registration error:", error);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <main class="min-h-screen bg-bg flex items-center justify-center p-4 relative overflow-hidden">
      <Title>Create Account - Web3Bank</Title>

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
            Create Account
          </h2>
          <p class="text-muted text-sm mb-8">
            Set up your account with passkey authentication.
          </p>

          {/* Error */}
          <Show when={auth.error()}>
            <div class="mb-6 p-3 bg-error/10 border border-error/20 rounded-xl">
              <p class="text-error text-sm">{auth.error()}</p>
            </div>
          </Show>

          <form onSubmit={handleRegister}>
            <div class="mb-6">
              <label
                for="displayName"
                class="block text-sm font-medium text-muted mb-2"
              >
                Display Name
              </label>
              <input
                type="text"
                id="displayName"
                value={displayName()}
                onInput={(e) => setDisplayName(e.currentTarget.value)}
                placeholder="Your name"
                class="w-full bg-raised border border-edge rounded-xl px-4 py-3 text-text placeholder-subtle focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/20 transition-all"
                disabled={isSubmitting()}
                required
              />
              <p class="text-subtle text-xs mt-1.5">
                This is how you'll appear in the app.
              </p>
            </div>

            <button
              type="submit"
              disabled={isSubmitting() || !displayName()}
              class="w-full bg-accent hover:bg-accent-hover active:scale-[0.98] text-accent-fg font-semibold py-3.5 px-4 rounded-xl transition-all disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center gap-2.5"
            >
              <Show
                when={!isSubmitting()}
                fallback={
                  <span class="flex items-center gap-2">
                    <svg class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
                      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="60" stroke-linecap="round" />
                    </svg>
                    Creating account...
                  </span>
                }
              >
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                </svg>
                Create Passkey
              </Show>
            </button>
          </form>

          {/* Info box */}
          <div class="mt-6 p-4 bg-accent/5 border border-accent/10 rounded-xl">
            <h3 class="text-sm font-medium text-accent mb-1.5 flex items-center gap-2">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              What is a passkey?
            </h3>
            <p class="text-muted text-xs leading-relaxed">
              A passkey uses your device's biometrics (Face ID, Touch ID) to create a secure
              cryptographic key. No passwords, no seed phrases.
            </p>
          </div>

          {/* Login link */}
          <div class="mt-6 text-center">
            <p class="text-subtle text-sm">
              Already have an account?{" "}
              <a
                href="/login"
                class="text-accent hover:text-accent-hover font-medium transition-colors"
              >
                Sign in
              </a>
            </p>
          </div>
        </div>

        <p class="text-subtle text-xs text-center mt-6">
          Your passkey is stored on your device and synced via your platform's keychain.
        </p>
      </div>
    </main>
  );
}
