import { Title } from "@solidjs/meta";
import { HttpStatusCode } from "@solidjs/start";

export default function NotFound() {
  return (
    <main class="min-h-screen bg-[#141414] flex items-center justify-center p-4">
      <Title>Not Found - Web3Bank</Title>
      <HttpStatusCode code={404} />
      <div class="text-center animate-in">
        <div class="text-6xl font-bold text-warm/10 font-[Satoshi] mb-4">
          404
        </div>
        <h1 class="text-xl font-bold text-white font-[Satoshi] mb-2">
          Page not found
        </h1>
        <p class="text-warm/50 text-sm mb-6">
          The page you're looking for doesn't exist.
        </p>
        <a
          href="/"
          class="inline-flex items-center gap-2 px-4 py-2.5 bg-hue hover:bg-hue/90 text-white text-sm font-medium rounded-xl transition-all active:scale-[0.98]"
        >
          Back to Dashboard
        </a>
      </div>
    </main>
  );
}
