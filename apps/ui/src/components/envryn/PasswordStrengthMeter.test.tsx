import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PasswordStrengthMeter } from "./PasswordStrengthMeter";

describe("PasswordStrengthMeter", () => {
  it("renders nothing until a password is entered", () => {
    const { container } = render(<PasswordStrengthMeter password="" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("labels a weak password and includes a useful suggestion", () => {
    render(<PasswordStrengthMeter password="password" />);
    expect(screen.getByRole("img", { name: /Estimated password strength:/ })).toBeInTheDocument();
    expect(screen.getByText(/--/)).toBeInTheDocument();
  });

  it("renders a strong password without requiring a suggestion", () => {
    render(<PasswordStrengthMeter password="correct horse battery staple 9!" />);
    expect(screen.getByRole("img", { name: /Estimated password strength:/ })).toBeInTheDocument();
  });
});
