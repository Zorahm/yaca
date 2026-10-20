import { describe, expect, it } from "vitest";

import { parseConnectionLink, socketAddress } from "./link";

describe("socketAddress", () => {
  it("takes a bare host as a secure server", () => {
    expect(socketAddress("yaca.example.com")).toBe("wss://yaca.example.com/ws");
  });

  it("maps https to wss and keeps the port", () => {
    expect(socketAddress("https://yaca.example.com:8443/")).toBe("wss://yaca.example.com:8443/ws");
  });

  it("leaves an address that already names the socket alone", () => {
    expect(socketAddress("wss://yaca.example.com/ws")).toBe("wss://yaca.example.com/ws");
  });

  it("keeps a path the proxy mounts the server under", () => {
    expect(socketAddress("https://example.com/yaca")).toBe("wss://example.com/yaca/ws");
  });

  it("allows a plain socket only when it is asked for in full", () => {
    expect(socketAddress("ws://192.168.1.5:7878")).toBe("ws://192.168.1.5:7878/ws");
    expect(socketAddress("http://192.168.1.5:7878")).toBe("ws://192.168.1.5:7878/ws");
  });

  it("refuses what cannot be an address", () => {
    expect(socketAddress("")).toBeNull();
    expect(socketAddress("   ")).toBeNull();
    expect(socketAddress("ftp://example.com")).toBeNull();
    expect(socketAddress("http://")).toBeNull();
  });
});

describe("parseConnectionLink", () => {
  it("reads the line the installer prints", () => {
    expect(parseConnectionLink("yaca://connect?host=yaca.example.com&token=yct_abc")).toEqual({
      address: "https://yaca.example.com",
      token: "yct_abc",
    });
  });

  it("marks a server without TLS as plain", () => {
    expect(parseConnectionLink("yaca://connect?host=10.0.0.5&token=yct_abc&plain=1")?.address).toBe(
      "http://10.0.0.5",
    );
  });

  it("ignores whatever is not such a link", () => {
    expect(parseConnectionLink("yaca.example.com")).toBeNull();
    expect(parseConnectionLink("https://example.com/?host=a&token=b")).toBeNull();
    expect(parseConnectionLink("yaca://connect?host=a")).toBeNull();
  });
});
