import { beforeEach, describe, expect, test, vi } from "vitest";

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
