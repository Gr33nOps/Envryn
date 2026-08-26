import { describe, expect, it } from "vitest";
import { navigationGroups } from "./Sidebar";
import type { Secret } from "@/lib/envryn-data";

function fakeSecret(overrides: Partial<Secret> = {}): Secret {
  return {
    id: crypto.randomUUID(),
    name: "SECRET",
    type: "API Key",
    project: "",
    environment: "—",
    updated: "just now",
    created: "today",
    value: "",
    ...overrides,
  };
}

describe("navigationGroups", () => {
  it("counts every secret under All secrets, regardless of type", () => {
    const secrets = [fakeSecret(), fakeSecret({ type: "Database" }), fakeSecret({ type: "Note" })];
    const groups = navigationGroups(secrets, 0);
    const allSecrets = groups[0]?.items.find((i) => i.label === "All secrets");
    expect(allSecrets?.count).toBe(3);
  });

  it("passes the project count straight through to the Projects item", () => {
    const groups = navigationGroups([], 4);
    const projects = groups[0]?.items.find((i) => i.label === "Projects");
    expect(projects?.count).toBe(4);
  });

  it("buckets API Key, Token, OAuth, and Webhook together under API & tokens", () => {
    const secrets = [
      fakeSecret({ type: "API Key" }),
      fakeSecret({ type: "Token" }),
      fakeSecret({ type: "OAuth" }),
      fakeSecret({ type: "Webhook" }),
      fakeSecret({ type: "Database" }), // must not be counted here
    ];
    const groups = navigationGroups(secrets, 0);
    const apiTokens = groups[0]?.items.find((i) => i.label === "API & tokens");
    expect(apiTokens?.count).toBe(4);
  });

  it("counts Database secrets separately from every other category", () => {
    const secrets = [fakeSecret({ type: "Database" }), fakeSecret({ type: "Database" })];
    const groups = navigationGroups(secrets, 0);
    expect(groups[0]?.items.find((i) => i.label === "Databases")?.count).toBe(2);
    expect(groups[0]?.items.find((i) => i.label === "SSH")?.count).toBe(0);
  });

  it("counts SSH and Secure notes independently", () => {
    const secrets = [
      fakeSecret({ type: "SSH" }),
      fakeSecret({ type: "Note" }),
      fakeSecret({ type: "Note" }),
    ];
    const groups = navigationGroups(secrets, 0);
    expect(groups[0]?.items.find((i) => i.label === "SSH")?.count).toBe(1);
    expect(groups[0]?.items.find((i) => i.label === "Secure notes")?.count).toBe(2);
  });

  it("always includes the Devices group with Trusted devices and Sync, uncounted", () => {
    const groups = navigationGroups([], 0);
    const devices = groups.find((g) => g.label === "Devices");
    expect(devices?.items.map((i) => i.label)).toEqual(["Trusted devices", "Sync"]);
  });
});
