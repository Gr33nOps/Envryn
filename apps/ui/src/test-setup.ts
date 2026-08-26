// Registers jest-dom's matchers (toBeInTheDocument, etc.) globally for
// every test file -- wired in via vite.config.ts's test.setupFiles.
import "@testing-library/jest-dom/vitest";

import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// React Testing Library's own automatic cleanup only registers itself when
// it detects a global `afterEach` (vitest's `test.globals` is deliberately
// off here, matching every other test file's explicit imports) -- without
// this, a component left mounted from one test (its effects still live)
// bleeds into the next test's assertions, which is exactly what happened
// while writing TitleBar.test.tsx: mounted instances piled up and their
// useEffects fired out of order against later tests' mocks.
afterEach(cleanup);
