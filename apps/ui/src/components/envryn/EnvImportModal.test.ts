import { describe, expect, it } from "vitest";
import { parseEnvText } from "./EnvImportModal";

describe("parseEnvText (adversarial: malformed .env content)", () => {
  it("returns an empty list for content with no KEY=VALUE lines, rather than throwing", () => {
    expect(() => parseEnvText("")).not.toThrow();
    expect(parseEnvText("")).toEqual([]);
    expect(parseEnvText("just some prose\nwith no equals signs anywhere")).toEqual([]);
  });

  it("skips lines that don't match the grammar instead of crashing on them", () => {
    const malformed = [
      "not-a-valid-line",
      "====",
      "",
      "# comment only",
      "NOVALUEHERE",
      "=".repeat(500),
      "=only-a-value",
      "   ",
      "-*+invalid+key+chars=value",
    ].join("\n");
    expect(() => parseEnvText(malformed)).not.toThrow();
    const result = parseEnvText(malformed);
    // None of the above lines are valid KEY=VALUE pairs, so nothing should
    // be extracted -- the important thing is that the parser degrades to
    // "found nothing" rather than throwing or extracting garbage.
    expect(result).toEqual([]);
  });

  it("handles a pathologically long single line without throwing or hanging", () => {
    const hugeValue = "A".repeat(1_000_000);
    const input = `HUGE_VAR=${hugeValue}`;
    const start = Date.now();
    const result = parseEnvText(input);
    const elapsed = Date.now() - start;
    expect(elapsed).toBeLessThan(1000);
    expect(result).toEqual([{ key: "HUGE_VAR", value: hugeValue }]);
  });

  it("handles embedded null-like and control characters without throwing", () => {
    // Actual NUL bytes cannot round-trip through a JS string the way a real
    // pasted clipboard value might carry them, but control characters and
    // other non-printable content are a realistic adversarial paste.
    const withControlChars = "KEY=valuewith control chars";
    expect(() => parseEnvText(withControlChars)).not.toThrow();
    const result = parseEnvText(withControlChars);
    expect(result).toEqual([{ key: "KEY", value: "valuewith control chars" }]);
  });

  it("parses genuinely valid lines correctly even when interleaved with malformed ones", () => {
    const mixed = [
      "not-valid",
      "GOOD_KEY=good-value",
      "====",
      'QUOTED="quoted value"',
      "# a comment",
      "ANOTHER_GOOD=123",
    ].join("\n");
    expect(parseEnvText(mixed)).toEqual([
      { key: "GOOD_KEY", value: "good-value" },
      { key: "QUOTED", value: "quoted value" },
      { key: "ANOTHER_GOOD", value: "123" },
    ]);
  });

  it("does not crash on a key with no value after the equals sign", () => {
    expect(() => parseEnvText("EMPTY_VALUE=")).not.toThrow();
    // An empty value is filtered out (both key and value must be non-empty
    // per the function's own contract), not stored as a blank secret.
    expect(parseEnvText("EMPTY_VALUE=")).toEqual([]);
  });
});
