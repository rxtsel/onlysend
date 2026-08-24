import {
    connectResend,
    disconnectResend,
    getConnectionStatus,
    getOnboardingState,
    type ConnectionMethod,
} from "@/lib/shared/api/auth";
import { getActiveDomain } from "@/lib/shared/api/domains";

/** Result of an action that may fail; UI decides how to report it. */
export type ActionResult = { ok: true } | { ok: false; error: string };

/**
 * Centralizes connection state shared by the sidebar identity and the
 * settings dialog: status, active domain, inbox flag, busy flags.
 */
export class ConnectionStore {
    status = $state<{ method: ConnectionMethod }>({ method: null });
    activeDomain = $state<string | null>(null);
    inboxEnabled = $state(false);
    isConnecting = $state(false);
    isDisconnecting = $state(false);

    async load(): Promise<void> {
        const [domain, status, onboarding] = await Promise.all([
            getActiveDomain(),
            getConnectionStatus(),
            getOnboardingState(),
        ]);
        this.activeDomain = domain;
        this.status = status;
        this.inboxEnabled = onboarding.inboxEnabled;
    }

    async connect(): Promise<ActionResult> {
        if (this.isConnecting) return { ok: true };
        try {
            this.isConnecting = true;
            await connectResend();
            await this.load();
            return { ok: true };
        } catch (err) {
            console.error("Error connecting with Resend:", err);
            return { ok: false, error: "Failed to start the connection" };
        } finally {
            this.isConnecting = false;
        }
    }

    async disconnect(): Promise<ActionResult> {
        if (this.isDisconnecting) return { ok: true };
        try {
            this.isDisconnecting = true;
            await disconnectResend();
            await this.load();
            return { ok: true };
        } catch (err) {
            console.error("Error disconnecting:", err);
            return { ok: false, error: "Failed to disconnect" };
        } finally {
            this.isDisconnecting = false;
        }
    }
}
