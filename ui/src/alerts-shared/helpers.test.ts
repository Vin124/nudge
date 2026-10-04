import { describe, expect, it } from "vitest";
import { phrase, safeColor } from "./helpers";

describe("phrase", () => {
  it("builds done / blocked / escalation phrases", () => {
    expect(phrase("api", "done", 0)).toBe("api is done");
    expect(phrase("api", "blocked", 0)).toBe("api needs you");
    expect(phrase("api", "done", 1)).toBe("api is still waiting");
    expect(phrase("api", "blocked", 2)).toBe("api is still waiting");
  });
});

describe("safeColor", () => {
  it("accepts hex and rejects everything else", () => {
    expect(safeColor("#3ddc84", "#000")).toBe("#3ddc84");
    expect(safeColor("#fff", "#000")).toBe("#fff");
    expect(safeColor("red; background:url(x)", "#000")).toBe("#000");
    expect(safeColor("", "#000")).toBe("#000");
  });
});
