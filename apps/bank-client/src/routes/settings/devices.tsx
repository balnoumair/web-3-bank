import { Title } from '@solidjs/meta';
import { createSignal, For, Show } from 'solid-js';
import { useCredentials, useAddCredential } from '~/hooks/use-credentials';
import { truncateAddress } from '~/lib/format';
import Skeleton from '~/components/Skeleton';

export default function DevicesSettings() {
  const credentials = useCredentials();
  const addCredential = useAddCredential();
  const [deviceLabel, setDeviceLabel] = createSignal('');
  const [error, setError] = createSignal<string | null>(null);
  const [success, setSuccess] = createSignal<string | null>(null);

  const handleAddDevice = async (e: Event) => {
    e.preventDefault();
    const label = deviceLabel().trim();
    if (!label) return;

    setError(null);
    setSuccess(null);
    try {
      const result = await addCredential.mutateAsync(label);
      setDeviceLabel('');
      setSuccess(
        `Passkey added. On-chain address for this device: ${truncateAddress(result.tempoAddress)}`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add passkey');
    }
  };

  return (
    <div class="max-w-2xl">
      <Title>Passkeys - Web3Bank</Title>

      <div class="mb-8">
        <h1 class="text-2xl font-bold text-text font-[Satoshi] tracking-tight">
          Passkeys
        </h1>
        <p class="text-muted text-sm mt-2">
          Manage the passkeys that can sign in to your account. Adding a device requires
          your existing passkey plus a fresh enrollment on the new device.
        </p>
      </div>

      <Show when={error()}>
        <div class="mb-6 p-3 bg-error/10 border border-error/20 rounded-xl">
          <p class="text-error text-sm">{error()}</p>
        </div>
      </Show>

      <Show when={success()}>
        <div class="mb-6 p-3 bg-accent/10 border border-accent/20 rounded-xl">
          <p class="text-accent text-sm">{success()}</p>
        </div>
      </Show>

      <section class="bg-surface border border-edge rounded-2xl p-6 mb-6">
        <h2 class="text-lg font-semibold text-text font-[Satoshi] mb-1">
          Your devices
        </h2>
        <p class="text-subtle text-sm mb-5">
          Each passkey is tied to a Tempo address derived from that device&apos;s key.
        </p>

        <Show
          when={!credentials.isLoading}
          fallback={
            <div class="space-y-3">
              <Skeleton class="h-14 w-full" />
              <Skeleton class="h-14 w-full" />
            </div>
          }
        >
          <Show
            when={(credentials.data?.length ?? 0) > 0}
            fallback={
              <p class="text-muted text-sm py-4">No passkeys found for this account.</p>
            }
          >
            <ul class="divide-y divide-edge/60">
              <For each={credentials.data}>
                {(cred) => (
                  <li class="py-4 flex items-start justify-between gap-4">
                    <div>
                      <div class="text-sm font-medium text-text font-mono">
                        {truncateAddress(cred.credentialId, 6)}
                      </div>
                      <div class="text-xs text-subtle mt-1">
                        Address {truncateAddress(cred.tempoAddress)} · Added{' '}
                        {new Date(cred.createdAt).toLocaleDateString('en-US', {
                          month: 'short',
                          day: 'numeric',
                          year: 'numeric',
                        })}
                      </div>
                    </div>
                    <span
                      class={`text-xs px-2 py-1 rounded-full ${
                        cred.revoked
                          ? 'bg-error/10 text-error'
                          : 'bg-accent/10 text-accent'
                      }`}
                    >
                      {cred.revoked ? 'Revoked' : 'Active'}
                    </span>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </Show>
      </section>

      <section class="bg-surface border border-edge rounded-2xl p-6">
        <h2 class="text-lg font-semibold text-text font-[Satoshi] mb-1">
          Add another device
        </h2>
        <p class="text-subtle text-sm mb-5">
          You&apos;ll confirm with your current passkey, then enroll a new one on this
          device (two biometric prompts).
        </p>

        <form onSubmit={handleAddDevice} class="space-y-4">
          <div>
            <label
              for="deviceLabel"
              class="block text-sm font-medium text-muted mb-2"
            >
              Device label
            </label>
            <input
              id="deviceLabel"
              type="text"
              value={deviceLabel()}
              onInput={(e) => setDeviceLabel(e.currentTarget.value)}
              placeholder="e.g. MacBook, iPhone"
              class="w-full bg-raised border border-edge rounded-xl px-4 py-3 text-text placeholder-subtle focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/20 transition-all"
              disabled={addCredential.isPending}
              required
            />
          </div>

          <button
            type="submit"
            disabled={addCredential.isPending || !deviceLabel().trim()}
            class="w-full bg-accent hover:bg-accent/90 disabled:opacity-50 disabled:cursor-not-allowed text-white font-medium py-3 px-4 rounded-xl transition-colors"
          >
            {addCredential.isPending ? 'Waiting for passkey…' : 'Add passkey on this device'}
          </button>
        </form>
      </section>
    </div>
  );
}
