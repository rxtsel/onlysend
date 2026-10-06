import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
    listDomains: vi.fn(),
    createDomain: vi.fn(),
    deleteDomain: vi.fn(),
    getDomain: vi.fn(),
    verifyDomain: vi.fn(),
    setDomainReceiving: vi.fn(),
    setInboxEnabled: vi.fn(),
}));

vi.mock("@/lib/shared/api/domains", () => ({ ...mocks }));
vi.mock("@/lib/shared/api/auth", () => ({ setInboxEnabled: mocks.setInboxEnabled }));

import {
    allGreen,
    canSelect,
    DomainSetupStore,
    needsDns,
    sendingReady,
    dnsVerificationPending,
} from "./domain-setup-store.svelte.js";
import type { DomainDetail, DomainSummary } from "@/lib/shared/api/domains";

function summary(overrides: Partial<DomainSummary> = {}): DomainSummary {
    return {
        id: "d1",
        name: "mail.rxtsel.dev",
        status: "verified",
        capabilities: { sending: "enabled", receiving: "disabled" },
        ...overrides,
    };
}

function detail(overrides: Partial<DomainDetail> = {}): DomainDetail {
    return {
        id: "d1",
        name: "mail.rxtsel.dev",
        status: "verified",
        capabilities: { sending: "enabled", receiving: "enabled" },
        records: [
            {
                group: "DKIM",
                recordType: "TXT",
                name: "x._domainkey.mail",
                value: "p=v",
                status: "verified",
                ttl: "Auto",
            },
        ],
        ...overrides,
    };
}

/* ---------------------------------------------------------
 * PURE RULES
 * --------------------------------------------------------- */
describe("pure rules", () => {
    test("canSelect accepts verified or sending-enabled domains", () => {
        expect(canSelect(summary({ status: "verified" }))).toBe(true);
        expect(
            canSelect(
                summary({
                    status: "partially_verified",
                    capabilities: { sending: "enabled", receiving: "disabled" },
                }),
            ),
        ).toBe(true);
        expect(
            canSelect(
                summary({ status: "not_started", capabilities: { sending: "disabled", receiving: "disabled" } }),
            ),
        ).toBe(false);
    });

    test("allGreen requires every listed record verified", () => {
        const d = detail();
        expect(allGreen(d)).toBe(true);

        const pending = detail({
            records: [d.records.map((r) => ({ ...r, status: "pending" }))[0]],
        });
        expect(allGreen(pending)).toBe(false);

        expect(allGreen(detail({ records: [] }))).toBe(false);
    });

    test("sending readiness does not confuse enabled capability with verified DNS", () => {
        expect(sendingReady(summary({ status: "pending" }))).toBe(false);
        expect(sendingReady(summary())).toBe(true);
        expect(sendingReady(detail({ records: [{ ...detail().records[0], status: "pending" }] }))).toBe(false);
        const mixed = detail({ status: "partially_verified", records: [
            detail().records[0],
            { ...detail().records[0], group: "Receiving MX", status: "pending" },
        ] });
        expect(sendingReady(mixed)).toBe(true);
    });

    test("DNS observation includes pending receiving records and aggregate status, but not disabled receiving", () => {
        const pendingMx = detail({ records: [detail().records[0], {
            ...detail().records[0], group: "Receiving MX", status: "pending",
        }] });
        expect(dnsVerificationPending(pendingMx)).toBe(true);
        expect(dnsVerificationPending({ ...pendingMx, capabilities: { sending: "enabled", receiving: "disabled" } })).toBe(false);
        expect(dnsVerificationPending(detail({ status: "pending" }))).toBe(true);
        expect(dnsVerificationPending(detail({ records: [] }))).toBe(false);
        expect(dnsVerificationPending(detail())).toBe(false);
    });

    test("needsDns matrix", () => {
        expect(needsDns(detail())).toBe(false);

        expect(needsDns(detail({ status: "partially_verified" }))).toBe(true);
        expect(
            needsDns(
                detail({
                    status: "partially_verified",
                    records: [
                        { ...detail().records[0], status: "pending" },
                    ],
                }),
            ),
        ).toBe(true);
        expect(
            needsDns(
                detail({
                    capabilities: { sending: "enabled", receiving: "disabled" },
                }),
            ),
        ).toBe(true);
    });
});

/* ---------------------------------------------------------
 * STORE
 * --------------------------------------------------------- */
describe("DomainSetupStore", () => {
    let store: DomainSetupStore;

    beforeEach(() => {
        vi.clearAllMocks();
        mocks.getDomain.mockImplementation(() =>
            Promise.resolve(detail()),
        );
        store = new DomainSetupStore("account-a");
    });

    afterEach(() => { store.dispose(); vi.useRealTimers(); });

    test("opening pending DNS is read-only, without automatic verification", async () => {
        mocks.getDomain.mockResolvedValue(detail({ status: "pending" }));
        await store.loadFresh("d1");
        expect(mocks.verifyDomain).not.toHaveBeenCalled();
        expect(mocks.setDomainReceiving).not.toHaveBeenCalled();
    });

    test("opening a pending panel refreshes records to verified without reopening or triggering verification", async () => {
        vi.useFakeTimers();
        mocks.getDomain.mockResolvedValueOnce(detail({
            status: "pending", records: [{ ...detail().records[0], status: "pending" }],
        })).mockResolvedValue(detail());
        await store.loadFresh("d1");
        expect(store.setupDetail?.records[0].status).toBe("pending");
        await vi.advanceTimersByTimeAsync(5000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(2);
        expect(mocks.getDomain).toHaveBeenLastCalledWith("account-a", "d1");
        expect(store.setupDetail?.records[0].status).toBe("verified");
        expect(mocks.verifyDomain).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(15_000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(2);
    });

    test("enabling receiving starts observing the pending MX even if sending is already verified", async () => {
        vi.useFakeTimers();
        store.applyDetail(detail({ capabilities: { sending: "enabled", receiving: "disabled" } }));
        const pendingMx = detail({ records: [detail().records[0], {
            ...detail().records[0], group: "Receiving MX", recordType: "MX", status: "pending",
        }] });
        mocks.setDomainReceiving.mockResolvedValue(pendingMx);
        mocks.getDomain.mockResolvedValue(detail({ records: pendingMx.records.map((record) => ({ ...record, status: "verified" })) }));
        await store.toggleReceiving(true);
        expect(store.setupDetail?.records[1].status).toBe("pending");
        await vi.advanceTimersByTimeAsync(5000);
        expect(store.setupDetail?.records[1].status).toBe("verified");
        expect(mocks.getDomain).toHaveBeenCalledExactlyOnceWith("account-a", "d1");
        expect(mocks.verifyDomain).not.toHaveBeenCalled();
    });

    test("DNS observation continues until the domain status catches up with verified records", async () => {
        vi.useFakeTimers();
        mocks.getDomain.mockResolvedValueOnce(detail({ status: "pending" })).mockResolvedValue(detail());
        await store.loadFresh("d1");
        expect(store.verified).toBe(true);
        expect(store.isPolling).toBe(true);
        await vi.advanceTimersByTimeAsync(5000);
        expect(store.setupDetail?.status).toBe("verified");
        expect(store.isPolling).toBe(false);
    });

    test("reloading pending DNS replaces the old timer instead of starting duplicate polling", async () => {
        vi.useFakeTimers();
        const pending = detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] });
        mocks.getDomain.mockResolvedValue(pending);
        await store.loadFresh("d1");
        await store.loadFresh("d1");
        expect(vi.getTimerCount()).toBe(1);
        mocks.getDomain.mockResolvedValue(detail());
        await vi.advanceTimersByTimeAsync(5000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(3);
        expect(store.isPolling).toBe(false);
        expect(vi.getTimerCount()).toBe(0);
    });

    test("transient polling errors preserve pending records and recover on the next check", async () => {
        vi.useFakeTimers();
        const pending = detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] });
        mocks.getDomain.mockResolvedValueOnce(pending)
            .mockRejectedValueOnce(new Error("temporarily offline"))
            .mockResolvedValue(detail());
        await store.loadFresh("d1");
        await vi.advanceTimersByTimeAsync(5000);
        expect(store.pollingError).toContain("temporarily offline");
        expect(store.setupDetail?.records[0].status).toBe("pending");
        expect(store.isPolling).toBe(true);
        await vi.advanceTimersByTimeAsync(5000);
        expect(store.setupDetail?.records[0].status).toBe("verified");
        expect(store.pollingError).toBeNull();
        expect(store.isPolling).toBe(false);
    });

    test("automatic DNS observation is bounded and can be restarted without reopening settings", async () => {
        vi.useFakeTimers();
        mocks.getDomain.mockResolvedValue(detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] }));
        await store.loadFresh("d1");
        await vi.advanceTimersByTimeAsync(300_000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(61);
        expect(store.pollingTimedOut).toBe(true);
        expect(store.isPolling).toBe(false);
        expect(vi.getTimerCount()).toBe(0);
        await vi.advanceTimersByTimeAsync(10_000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(61);
        mocks.getDomain.mockResolvedValue(detail());
        await store.loadFresh("d1");
        expect(store.pollingTimedOut).toBe(false);
        expect(store.setupDetail?.records[0].status).toBe("verified");
    });

    test("closing an automatically observed panel ignores in-flight DNS responses and stops timers", async () => {
        vi.useFakeTimers();
        const pending = detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] });
        let resolve!: (value: DomainDetail) => void;
        mocks.getDomain.mockResolvedValueOnce(pending)
            .mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
        await store.loadFresh("d1");
        await vi.advanceTimersByTimeAsync(5000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(2);
        store.dispose();
        resolve(detail());
        await vi.advanceTimersByTimeAsync(20_000);
        expect(store.setupDetail?.records[0].status).toBe("pending");
        expect(store.isPolling).toBe(false);
        expect(vi.getTimerCount()).toBe(0);
        expect(mocks.getDomain).toHaveBeenCalledTimes(2);
    });

    test("slow domain A cannot overwrite newer domain B", async () => {
        let resolve!: (value: DomainDetail) => void;
        mocks.getDomain.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
        const pending = store.loadFresh("d1");
        mocks.getDomain.mockResolvedValueOnce(detail({ id: "d2", name: "second.example" }));
        await store.loadFresh("d2");
        resolve(detail());
        await pending;
        expect(store.setupDetail?.id).toBe("d2");
    });

    test("closing a panel during verification discards the response and does not restart polling", async () => {
        vi.useFakeTimers();
        let resolve!: (value: DomainDetail) => void;
        store.applyDetail(detail());
        mocks.verifyDomain.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
        const pending = store.verify();
        store.dispose();
        resolve(detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] }));
        await pending;
        await vi.advanceTimersByTimeAsync(15_000);
        expect(store.setupDetail?.status).toBe("verified");
        expect(mocks.getDomain).not.toHaveBeenCalled();
    });

    test("verification polling never overlaps slow requests", async () => {
        vi.useFakeTimers();
        const pendingDetail = detail({ status: "pending", records: [{ ...detail().records[0], status: "pending" }] });
        store.applyDetail(pendingDetail);
        mocks.verifyDomain.mockResolvedValue(pendingDetail);
        let resolve!: (value: DomainDetail) => void;
        mocks.getDomain.mockImplementation(() => new Promise((done) => { resolve = done; }));
        await store.verify();
        await vi.advanceTimersByTimeAsync(20_000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(1);
        resolve(detail());
        await vi.advanceTimersByTimeAsync(20_000);
        expect(mocks.getDomain).toHaveBeenCalledTimes(1);
        expect(store.verified).toBe(true);
    });

    test("load fetches the domain list", async () => {
        mocks.listDomains.mockResolvedValue([summary()]);
        await store.load();

        expect(store.domains).toHaveLength(1);
        expect(store.isLoading).toBe(false);
    });

    test("loadFresh reports no DNS needed when fully ready", async () => {
        const result = await store.loadFresh("d1");

        expect(result.needsDns).toBe(false);
        expect(store.setupDetail?.status).toBe("verified");
    });

    test("loadFresh flags DNS view when something is pending", async () => {
        mocks.getDomain.mockResolvedValue(
            detail({
                status: "partially_verified",
                records: [{ ...detail().records[0], status: "pending" }],
            }),
        );

        const { detail: loaded, needsDns } = await store.loadFresh("d1");

        expect(needsDns).toBe(true);
        expect(loaded.records[0].status).toBe("pending");
    });

    test("create applies the new domain detail", async () => {
        const created = detail({ id: "d2", name: "new.dev", status: "not_started" });
        created.records = [];
        mocks.createDomain.mockResolvedValue(created);

        await store.create({ name: "new.dev", region: "us-east-1", enableReceiving: false });

        expect(mocks.createDomain).toHaveBeenCalledOnce();
        expect(store.setupDetail?.name).toBe("new.dev");
        expect(store.isCreating).toBe(false);
    });

    test("verify triggers the verification cycle", async () => {
        const pending = detail({
            status: "pending",
            records: [{ ...detail().records[0], status: "pending" }],
        });
        store.applyDetail(pending);
        // The command returns the refreshed detail (verify + get_domain).
        mocks.verifyDomain.mockResolvedValue(
            detail({ status: "pending", records: pending.records }),
        );

        await store.verify();

        expect(mocks.verifyDomain).toHaveBeenCalledWith("account-a", "d1");
        expect(store.isVerifying).toBe(false);
        // Stop the bounded polling so the test ends deterministically.
        store.stopWork();
    });

    test("toggleReceiving updates detail through the API", async () => {
        store.applyDetail(detail());
        const disabled = detail({
            capabilities: { sending: "enabled", receiving: "disabled" },
        });
        mocks.setDomainReceiving.mockResolvedValue(disabled);

        await store.toggleReceiving(false);

        expect(mocks.setDomainReceiving).toHaveBeenCalledWith("account-a", "d1", false);
        expect(store.setupDetail?.capabilities.receiving).toBe("disabled");
    });

    test("remove delegates to deleteDomain", async () => {
        mocks.deleteDomain.mockResolvedValue(true);

        const result = await store.remove("d1");

        expect(result).toBe(true);
        expect(mocks.deleteDomain).toHaveBeenCalledWith("account-a", "d1");
    });
});
