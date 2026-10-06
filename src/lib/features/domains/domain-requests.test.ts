import { expect, test, vi } from "vitest";
import { queueDomainRequest } from "./domain-requests";

test("DNS requests across panels have bounded concurrency and release slots on errors", async () => {
  const finish: Array<(value: number) => void> = [];
  const request = vi.fn(() => new Promise<number>((done) => { finish.push(done); }));
  const first = queueDomainRequest(request);
  const second = queueDomainRequest(request);
  const third = queueDomainRequest(request);
  expect(request).toHaveBeenCalledTimes(2);
  finish[0](1);
  expect(await first).toBe(1);
  await Promise.resolve();
  expect(request).toHaveBeenCalledTimes(3);
  finish[1](2);
  finish[2](3);
  expect(await Promise.all([second, third])).toEqual([2, 3]);
  await expect(queueDomainRequest(async () => { throw new Error("permissions"); })).rejects.toThrow("permissions");
  expect(await queueDomainRequest(async () => "next account")).toBe("next account");
});
