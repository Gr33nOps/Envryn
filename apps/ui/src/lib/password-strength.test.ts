import { describe, expect, it } from "vitest";
import { estimatePasswordStrength } from "./password-strength";

describe("estimatePasswordStrength", () => {
  it("scores an empty password with no label and no suggestions", () => {
    const result = estimatePasswordStrength("");
    expect(result.score).toBe(0);
    expect(result.label).toBe("");
    expect(result.suggestions).toEqual([]);
  });

  it("flags a top-of-breach-list password as very weak even at 8+ characters", () => {
    const result = estimatePasswordStrength("password1");
    expect(result.score).toBe(0);
    expect(result.label).toBe("Very weak");
  });

  it("flags a common password regardless of case", () => {
    expect(estimatePasswordStrength("Password1").score).toBe(0);
  });

  it("scores a short single-charset password as very weak or weak", () => {
    const result = estimatePasswordStrength("abcdefgh");
    expect(result.score).toBeLessThanOrEqual(1);
  });

  it("penalises a long repeated-character run", () => {
    const withRun = estimatePasswordStrength("aaaa3f9p2m8q");
    const withoutRun = estimatePasswordStrength("k3f9p2m8q1x7");
    expect(withRun.score).toBeLessThan(withoutRun.score);
  });

  it("penalises an obvious sequential run", () => {
    const sequential = estimatePasswordStrength("abcd1234EFGH");
    expect(sequential.suggestions.some((s) => s.includes("sequences"))).toBe(true);
  });

  it("scores a long, high-entropy random password as strong", () => {
    const result = estimatePasswordStrength("qR7$mZ2!vK9#xL4@wT6&");
    expect(result.score).toBe(4);
    expect(result.label).toBe("Strong");
  });

  it("gives higher scores to longer passwords with the same charset diversity", () => {
    const short = estimatePasswordStrength("aB3!aB3!");
    const long = estimatePasswordStrength("aB3!aB3!aB3!aB3!aB3!");
    expect(long.score).toBeGreaterThanOrEqual(short.score);
  });

  it("suggests mixing character classes for an all-lowercase password", () => {
    const result = estimatePasswordStrength("correcthorsebatterysta");
    expect(result.suggestions.some((s) => s.includes("Mix in"))).toBe(true);
  });
});
