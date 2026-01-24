import { Title } from "@solidjs/meta";
import { createSignal, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "~/contexts/auth-context";

export default function Register() {
    const navigate = useNavigate();
    const auth = useAuth();
    const [username, setUsername] = createSignal("");
    const [displayName, setDisplayName] = createSignal("");
    const [isSubmitting, setIsSubmitting] = createSignal(false);

    const handleRegister = async (e: Event) => {
        e.preventDefault();

        if (!username() || !displayName()) {
            return;
        }

        setIsSubmitting(true);

        try {
            await auth.register(username(), displayName());
            navigate("/");
        } catch (error) {
            // Error is handled by auth context
            console.error("Registration error:", error);
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <main class="min-h-screen bg-[#0a0a0a] flex items-center justify-center p-4">
            <Title>Create Account - Web3Bank</Title>

            <div class="w-full max-w-md">
                {/* Logo/Header */}
                <div class="text-center mb-8">
                    <h1 class="text-4xl font-bold text-white mb-2">Web3Bank</h1>
                    <p class="text-gray-400">Simplified Stablecoin Banking</p>
                </div>

                {/* Register Card */}
                <div class="bg-[#1f1f1f] border border-white/[0.06] rounded-2xl p-8">
                    <h2 class="text-2xl font-semibold text-white mb-2">Create Account</h2>
                    <p class="text-gray-400 text-sm mb-6">
                        Set up your account with passkey authentication
                    </p>

                    {/* Error Message */}
                    <Show when={auth.error()}>
                        <div class="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
                            <p class="text-red-400 text-sm">{auth.error()}</p>
                        </div>
                    </Show>

                    {/* Registration Form */}
                    <form onSubmit={handleRegister}>
                        <div class="mb-4">
                            <label for="username" class="block text-sm font-medium text-gray-300 mb-2">
                                Username
                            </label>
                            <input
                                type="text"
                                id="username"
                                value={username()}
                                onInput={(e) => setUsername(e.currentTarget.value)}
                                placeholder="Choose a username"
                                class="w-full bg-[#242424] border border-white/[0.06] rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-[#2d5f4d] focus:border-transparent transition-all"
                                disabled={isSubmitting()}
                                required
                            />
                            <p class="text-gray-500 text-xs mt-1">
                                This will be your unique identifier
                            </p>
                        </div>

                        <div class="mb-6">
                            <label for="displayName" class="block text-sm font-medium text-gray-300 mb-2">
                                Display Name
                            </label>
                            <input
                                type="text"
                                id="displayName"
                                value={displayName()}
                                onInput={(e) => setDisplayName(e.currentTarget.value)}
                                placeholder="Your full name"
                                class="w-full bg-[#242424] border border-white/[0.06] rounded-lg px-4 py-3 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-[#2d5f4d] focus:border-transparent transition-all"
                                disabled={isSubmitting()}
                                required
                            />
                            <p class="text-gray-500 text-xs mt-1">
                                This is how you'll be identified in the app
                            </p>
                        </div>

                        <button
                            type="submit"
                            disabled={isSubmitting() || !username() || !displayName()}
                            class="w-full bg-gradient-to-r from-[#2d5f4d] to-[#1f4939] hover:from-[#357059] hover:to-[#24563f] text-white font-semibold py-3 px-4 rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        >
                            <Show when={!isSubmitting()} fallback={<span>Creating Account...</span>}>
                                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                                </svg>
                                Create Passkey
                            </Show>
                        </button>
                    </form>

                    {/* Info Box */}
                    <div class="mt-6 p-4 bg-[#242424] border border-white/[0.06] rounded-lg">
                        <h3 class="text-sm font-medium text-white mb-2 flex items-center gap-2">
                            <svg class="w-4 h-4 text-[#10b981]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                            </svg>
                            What is a passkey?
                        </h3>
                        <p class="text-gray-400 text-xs">
                            A passkey is a secure, passwordless way to sign in using your device's biometric authentication (Face ID, Touch ID, or fingerprint). It's more secure than passwords and never leaves your device.
                        </p>
                    </div>

                    {/* Login Link */}
                    <div class="mt-6 text-center">
                        <p class="text-gray-400 text-sm">
                            Already have an account?{" "}
                            <a href="/login" class="text-[#10b981] hover:text-[#059669] font-medium transition-colors">
                                Sign in
                            </a>
                        </p>
                    </div>
                </div>

                {/* Security Note */}
                <div class="mt-6 text-center">
                    <p class="text-gray-500 text-xs">
                        🔒 Your passkey is stored securely on your device and synced via your platform's keychain.
                    </p>
                </div>
            </div>
        </main>
    );
}
