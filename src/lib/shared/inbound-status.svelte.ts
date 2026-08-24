/**
 * Cross-component signal: true when the active domain has receiving
 * enabled AND its MX record is verified. The inbox root page computes
 * it; the sidebar consumes it to skip API calls that would fail.
 */
export const inboundStatus = $state({ ready: false });
