// Shared across DNS panels: opening/verifying many domains must not create
// unbounded parallel API traffic. Credentials and IDs remain in each closure.
const pending: Array<() => Promise<void>> = [];
let active = 0;
const MAX_CONCURRENT = 2;

function drain(): void {
  while (active < MAX_CONCURRENT && pending.length) {
    const next = pending.shift()!;
    active += 1;
    void next();
  }
}

export function queueDomainRequest<T>(request: () => Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    pending.push(async () => {
      try { resolve(await request()); }
      catch (error) { reject(error); }
      finally { active -= 1; drain(); }
    });
    drain();
  });
}
