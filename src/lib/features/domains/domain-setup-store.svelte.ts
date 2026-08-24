import {
    createDomain,
    deleteDomain,
    getDomain,
    listDomains,
    setDomainReceiving,
    verifyDomain,
    type DomainDetail,
    type DomainSummary,
} from "@/lib/shared/api/domains";
import { setInboxEnabled } from "@/lib/shared/api/auth";
import type { DomainSummary as _DomainSummary } from "@/lib/shared/api/domains";

const POLL_INTERVAL_MS = 5000;
const POLL_MAX_ATTEMPTS = 60;

/** A domain is selectable once sending works for it. */
export function canSelect(d: DomainSummary): boolean {
    return d.status === "verified" || d.capabilities?.sending === "enabled";
}

/** Every listed record reached a green terminal status. */
export function allGreen(d: DomainDetail): boolean {
    return (
        d.records.length > 0 &&
        d.records.every((r) => r.status === "verified")
    );
}

/**
 * Continue routes through DNS whenever something is pending: unverified
 * records or receiving not enabled yet. Fully-green domains skip.
 */
export function needsDns(d: DomainDetail): boolean {
    return d.status !== "verified" || !allGreen(d) || d.capabilities.receiving !== "enabled";
}

export type { DomainSummary };

export class DomainSetupStore {
    domains = $state<DomainSummary[]>([]);
    isLoading = $state(true);
    /** Name + Resend id of the currently selected domain. */
    selectedName = $state("");
    selectedId = $state("");

    setupDetail = $state<DomainDetail | null>(null);
    verified = $state(false);

    isCreating = $state(false);
    isVerifying = $state(false);
    isDeleting = $state(false);
    isTogglingReceiving = $state(false);

    #pendingFlow: {
        state: string;
        codeVerifier: string;
    } | null = null;

    #pollTimer: ReturnType<typeof setInterval> | undefined = undefined;
    #pollAttempts = 0;

    get activeList() {
        return this.domains;
    }

    async load(): Promise<void> {
        this.isLoading = true;
        try {
            this.domains = await listDomains();
        } finally {
            this.isLoading = false;
        }
    }

    select(name: string, id: string) {
        this.selectedName = name;
        this.selectedId = id;
    }

    /** Clears the setup detail so the create form shows again. */
    resetSetup() {
        this.setupDetail = null;
        this.verified = false;
    }

    /**
     * Loads fresh detail for an existing domain id and reports whether the
     * DNS view should be shown (something pending).
     */

    async create(options: {
        name: string;
        region: string;
        enableReceiving: boolean;
    }): Promise<void> {
        this.isCreating = true;
        try {
            const created = await createDomain(options);
            this.applyDetail(created);
            this.selectedName = created.name;
        } finally {
            this.isCreating = false;
        }
    }

    /** Loads fresh detail for an existing domain and reports pending state. */
    async loadFresh(id: string): Promise<{ detail: DomainDetail; needsDns: boolean }> {
        const detail = await getDomain(id);
        this.applyDetail(detail);
        const dns = needsDns(detail);
        if (dns) this.maybeAutoVerify();
        return { detail, needsDns: dns };
    }

    async remove(id: string): Promise<boolean> {
        this.isDeleting = true;
        try {
            return await deleteDomain(id);
        } finally {
            this.isDeleting = false;
        }
    }

    async verify(): Promise<void> {
        if (!this.setupDetail || this.isVerifying) return;

        try {
            this.isVerifying = true;
            this.applyDetail(await verifyDomain(this.setupDetail.id));
            this.#startPolling();
        } finally {
            this.isVerifying = false;
        }
    }

    async toggleReceiving(enable: boolean): Promise<void> {
        if (!this.setupDetail || this.isTogglingReceiving) return;

        try {
            this.isTogglingReceiving = true;
            this.applyDetail(await setDomainReceiving(this.setupDetail.id, enable));
        } finally {
            this.isTogglingReceiving = false;
        }
    }

    stopWork() {
        this.#stopPolling();
    }

    /** Marks the list as initialized without fetching (error fallback). */
    markLoaded() {
        this.isLoading = false;
    }

    /**
     * Replicates the dashboard refresh: re-trigger verification once when
     * entering with pending records. Skipped for brand-new domains
     * (nothing to verify until DNS records exist) and already-green ones.
     */
    maybeAutoVerify() {
        if (
            !this.setupDetail ||
            this.verified ||
            this.setupDetail.status === "not_started" ||
            allGreen(this.setupDetail)
        ) {
            return;
        }
        this.verify();
    }

    applyDetail(detail: DomainDetail | undefined | null) {
        if (!detail) return;
        this.setupDetail = detail;
        this.verified = allGreen(detail);

        if (allGreen(detail)) {
            this.#stopPolling();
        }
    }

    #startPolling() {
        this.#pollAttempts = 0;
        this.stopWork();

        this.#pollTimer = setInterval(async () => {
            this.#pollAttempts += 1;
            if (!this.setupDetail || this.#pollAttempts > POLL_MAX_ATTEMPTS) {
                this.stopWork();
                return;
            }

            try {
                const fresh = await getDomain(this.setupDetail.id);
                if (fresh) this.applyDetail(fresh);
            } catch (err) {
                console.error(err);
            }
        }, POLL_INTERVAL_MS);
    }

    #stopPolling() {
        if (this.#pollTimer) {
            clearInterval(this.#pollTimer);
            this.#pollTimer = undefined;
        }
    }
}
