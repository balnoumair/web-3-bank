import { Title } from "@solidjs/meta";
import { createSignal, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "~/contexts/auth-context";

export default function Login() {
    const navigate = useNavigate();
    const auth = useAuth();
    const [username, setUsername] = createSignal("");
    const [isSubmitting, setIsSubmitting] = createSignal(false);

    const handleLogin = async (e: Event) => {
        e.preventDefault();
        setIsSubmitting(true);

        try {
            await auth.login(username() || undefined);
            navigate("/");
        } catch (error) {
            // Error is handled by auth context
            console.error("Login error:", error);
        } finally {
            setIsSubmitting(false);
        }
    };

    const handlePasskeyLogin = async () => {
        setIsSubmitting(true);
        try {
            await auth.login();
            navigate("/");
        } catch (error) {
            console.error("Passkey login error:", error);
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <main class="min-h-screen bg-[#0a0a0a] flex items-center justify-center p-4">
            <Title>Sign In - Web3Bank</Title>

            <div class="w-full max-w-md">
                {/* Logo/Header */}
                <div class="text-center mb-8">
                    <h1 class="text-4xl font-bold text-white mb-2">Web3Bank</h1>
                    <p class="text-gray-400">Simplified Stablecoin Banking</p>
                </div>

                {/* Login Card */}
                <div class="bg-[#1f1f1f] border border-white/[0.06] rounded-2xl p-8">
                    <h2 class="text-2xl font-semibold text-white mb-6">Sign In</h2>

                    {/* Error Message */}
                    <Show when={auth.error()}>
                        <div class="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
                            <p class="text-red-400 text-sm">{auth.error()}</p>
                        </div>
                    </Show>

                    {/* Passkey Login (Primary) */}
                    <button
                        onClick={handlePasskeyLogin}
                        disabled={isSubmitting()}
                        class="w-full bg-gradient-to-r from-[#2d5f4d] to-[#1f4939] hover:from-[#357059] hover:to-[#24563f] text-white font-semibold py-3 px-4 rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 mb-4"
                    >
                        <Show when={!isSubmitting()} fallback={<span>Authenticating...</span>}>
                            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                            </svg>
                            Sign in with Passkey
                        </Show>
                    </button>

                    {/* Divider */}
                    <div class="relative my-6">
                        <div class="absolute inset-0 flex items-center">
                            <div class="w-full border-t border-white/[0.06]"></div>
                        </div>
                        <div class="relative flex justify-center text-sm">
                            <span class="px-2 bg-[#1f1f1f] text-gray-500">Or sign in with username</span>
                        </div>
                    </div>

                    {/* Username Login Form */}
                    <form onSubmit={handleLogin}>
                        <div class="mb-4">
                            <label for="username" class="block text-sm font-medium text-gray-300 mb-2">
                                Username
                            </label>
                            <input
                                type="text"
                                id="username"
                                value={username()}
                                onInput={(e) => setUsername(e.currentTarget.value)}
                                placeholder="Enter your username"
                                class="w-full bg-[#242424] border border-white/[0.06] rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-[#2d5f4d] focus:border-transparent transition-all"
                                disabled={isSubmitting()}
                            />
                        </div>

                        <button
                            type="submit"
                            disabled={isSubmitting() || !username()}
                            class="w-full bg-[#242424] hover:bg-[#2a2a2a] text-white font-semibold py-3 px-4 rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed border border-white/[0.06]"
                        >
                            <Show when={!isSubmitting()} fallback={<span>Authenticating...</span>}>
                                Continue
                            </Show>
                        </button>
                    </form>

                    {/* Register Link */}
                    <div class="mt-6 text-center">
                        <p class="text-gray-400 text-sm">
                            Don't have an account?{" "}
                            <a href="/register" class="text-[#10b981] hover:text-[#059669] font-medium transition-colors">
                                Create one
                            </a>
                        </p>
                    </div>
                </div>

                {/* Security Note */}
                <div class="mt-6 text-center">
                    <p class="text-gray-500 text-xs">
                        🔒 Secured with passkey authentication. Your biometric data never leaves your device.
                    </p>
                </div>
            </div>
        </main>
    );
}
