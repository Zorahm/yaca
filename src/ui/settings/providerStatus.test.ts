import { describe, expect, it } from "vitest";
import type { ProviderSpec } from "../../core/client";
import { providerAddress, providerStatus, pollAge, pollErrorLabel } from "./providerStatus";

const spec: ProviderSpec = { id: "test", displayName: "Test", protocol: "openai", baseUrl: "http://localhost:8080", headers: {}, models: [] };
const oldModels = { ...spec, models: [{ id: "cached" }] } as ProviderSpec;

describe("provider readiness", () => {
  it("requires credentials even with a cached model list", () => {
    expect(providerStatus(oldModels, { kind: "none" }, { checkedAt: 1, error: null })).toBe("missing_key");
  });
  it("keeps failed refreshes distinct from a ready cached list", () => {
    expect(providerStatus(oldModels, { kind: "keychain" }, { checkedAt: 1, error: "HTTP 401" })).toBe("error");
    expect(providerStatus(oldModels, { kind: "keychain" }, undefined, false, "network: offline")).toBe("error");
  });
  it("distinguishes no lookup from an empty answer for a keyless server", () => {
    expect(providerStatus(spec, { kind: "not_needed" }, undefined)).toBe("unasked");
    expect(providerStatus(spec, { kind: "not_needed" }, { checkedAt: 1, error: null })).toBe("empty");
  });
  it("never treats an unknown key source as a missing key", () => {
    expect(providerStatus(spec, { kind: "unknown" }, undefined)).toBe("unknown");
    expect(providerStatus(spec, { kind: "loading" }, undefined)).toBe("loading");
    expect(providerStatus(spec, { kind: "none" }, undefined, true)).toBe("asking");
  });
});

describe("provider metadata", () => {
  it.each(["http://localhost:8888", "http://127.0.0.2:8080/v1", "http://[::1]:8080", "http://api.localhost:8080"])("recognizes loopback addresses: %s", (url) => {
    expect(providerAddress(url).local).toBe(true);
  });
  it("does not mistake a remote host containing localhost for a local server", () => {
    expect(providerAddress("https://localhost.example.com/v1")).toEqual({ host: "localhost.example.com", local: false });
    expect(providerAddress("https://generativelanguage.googleapis.com/v1beta/openai").host).toBe("generativelanguage.googleapis.com");
  });
  it("keeps relative timestamps meaningful across minute, hour and day boundaries", () => {
    expect(pollAge(60_000, 0)).toBe(pollAge(0, 0));
    expect(pollAge(1, 300_001)).not.toBe(pollAge(0, 0));
    expect(pollAge(1, 3_600_001)).not.toBe(pollAge(1, 300_001));
    expect(pollAge(1, 86_400_001)).not.toBe(pollAge(1, 3_600_001));
  });
  it("gives authentication and connection failures different visible labels", () => {
    expect(pollErrorLabel("HTTP 401: bad token")).not.toBe(pollErrorLabel("network: connection refused"));
    expect(pollErrorLabel("HTTP 403: denied")).not.toBe(pollErrorLabel("HTTP 401: bad token"));
  });
});
