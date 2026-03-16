// ── Transaction simulation port (dependency injection boundary) ────────

export type SimulationRequest = {
    /** Chain ID as string, e.g. "84532" for Base Sepolia. */
    networkId: string;
    /** Publisher address that would submit the transaction. */
    from: string;
    /** Receiver contract address. */
    to: string;
    /** ABI-encoded calldata for publishRoute(...). */
    input: string;
    /** Gas limit to use for simulation. */
    gas: number;
    /** Native token value in wei. */
    value: number;
};

export type SimulationResult = {
    /** Whether simulation predicts the call will succeed. */
    success: boolean;
    /** Simulation identifier when available. */
    simulationId: string | null;
    /** Simulation trace URL when available. */
    traceUrl: string | null;
    /** Estimated gas used from simulation. */
    gasUsed: number;
    /** Error message when simulation predicts failure. */
    errorMessage: string | null;
};

export interface TransactionSimulationPort {
    simulate(req: SimulationRequest): Promise<SimulationResult>;
}
