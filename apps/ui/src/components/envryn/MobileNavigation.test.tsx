import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MobileNavigation } from "./MobileNavigation";

const controls = vi.hoisted(() => ({
  openAdd: vi.fn(),
  path: "/vault",
}));

vi.mock("@tanstack/react-router", () => ({
  Link: ({ children, to, ...props }: React.PropsWithChildren<{ to: string }>) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
  useRouterState: () => controls.path,
}));

vi.mock("@/components/ui/drawer", () => ({
  Drawer: ({ children, open }: React.PropsWithChildren<{ open: boolean }>) => (
    <div data-open={open}>{children}</div>
  ),
  DrawerClose: ({ children }: React.PropsWithChildren) => <>{children}</>,
  DrawerContent: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  DrawerDescription: ({ children }: React.PropsWithChildren) => <p>{children}</p>,
  DrawerTitle: ({ children }: React.PropsWithChildren) => <h2>{children}</h2>,
}));

vi.mock("./vault-context", () => ({
  useVaultUI: () => ({ openAdd: controls.openAdd }),
}));

beforeEach(() => {
  controls.openAdd.mockClear();
  controls.path = "/vault";
});

describe("MobileNavigation", () => {
  it("marks the exact vault route active and opens the add flow", () => {
    render(<MobileNavigation onLock={() => {}} />);
    expect(screen.getByRole("link", { name: "Secrets" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Projects" })).not.toHaveAttribute("aria-current");
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(controls.openAdd).toHaveBeenCalledTimes(1);
  });

  it("opens the mobile drawer and exposes categories and vault controls", () => {
    const onLock = vi.fn();
    render(<MobileNavigation onLock={onLock} />);
    fireEvent.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByText("API & tokens")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Sync/ })).toHaveAttribute("href", "/vault/sync");
    fireEvent.click(screen.getByRole("button", { name: /Lock vault/ }));
    expect(onLock).toHaveBeenCalledTimes(1);
  });

  it("uses prefix matching for a nested primary route", () => {
    controls.path = "/vault/projects/acme";
    render(<MobileNavigation onLock={() => {}} />);
    expect(screen.getByRole("link", { name: "Projects" })).toHaveAttribute("aria-current", "page");
  });
});
