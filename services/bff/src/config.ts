export function isDevMode(): boolean {
  return (
    process.env.BFF_DEV_MODE === "1" ||
    process.env.NODE_ENV === "development"
  );
}

export function assertBffConfig(): void {
  if (!process.env.JWT_SECRET && !isDevMode()) {
    console.error(
      "JWT_SECRET is required. Set BFF_DEV_MODE=1 only for local development.",
    );
    process.exit(1);
  }
}

export function getWebAuthnConfig() {
  return {
    rpID: process.env.WEBAUTHN_RP_ID ?? "localhost",
    origin: process.env.WEBAUTHN_ORIGIN ?? "http://localhost:3000",
    challengeTtlMs: 60_000,
  };
}
