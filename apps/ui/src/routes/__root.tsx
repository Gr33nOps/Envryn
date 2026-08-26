import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Outlet, Link, createRootRouteWithContext, useRouter } from "@tanstack/react-router";
import { Toaster } from "sonner";
import { TitleBar } from "@/components/envryn/TitleBar";
import { ResizeBorders } from "@/components/envryn/ResizeBorders";

function NotFoundComponent() {
  return (
    <div className="flex min-h-full items-center justify-center bg-background px-4">
      <div className="max-w-md text-center">
        <h1 className="text-7xl font-bold text-foreground">404</h1>
        <h2 className="mt-4 text-xl font-semibold text-foreground">Page not found</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          This screen does not exist or has been moved.
        </p>
        <div className="mt-6">
          <Link
            to="/"
            className="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            Go to unlock
          </Link>
        </div>
      </div>
    </div>
  );
}

/**
 * Root error boundary.
 *
 * Deliberately does not render `error.message`. A vault error can carry a
 * record name, a file path, or a fragment of a decrypted value, and the error
 * screen is exactly the surface a user screenshots and pastes into a chat.
 * The detail goes to the console for local debugging and no further -- Envryn
 * uploads nothing (see THREAT_MODEL.md, V-10).
 */
function ErrorComponent({ error, reset }: Readonly<{ error: Error; reset: () => void }>) {
  console.error(error);
  const router = useRouter();

  return (
    <div className="flex min-h-full items-center justify-center bg-background px-4">
      <div className="max-w-md text-center">
        <h1 className="text-xl font-semibold tracking-tight text-foreground">
          This screen did not load
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Your vault is unaffected and remains encrypted. Try again, or lock and reopen Envryn.
        </p>
        <div className="mt-6 flex flex-wrap justify-center gap-2">
          <button
            type="button"
            onClick={() => {
              router.invalidate();
              reset();
            }}
            className="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
          >
            Try again
          </button>
        </div>
      </div>
    </div>
  );
}

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  component: RootComponent,
  notFoundComponent: NotFoundComponent,
  errorComponent: ErrorComponent,
});

function RootComponent() {
  const { queryClient } = Route.useRouteContext();

  return (
    <QueryClientProvider client={queryClient}>
      <div className="flex h-screen flex-col overflow-hidden">
        <TitleBar />
        <div className="min-h-0 flex-1">
          {/* Required: nested routes render here. Removing <Outlet /> breaks all child routes. */}
          <Outlet />
        </div>
      </div>
      <ResizeBorders />
      <Toaster
        position="bottom-right"
        duration={2600}
        toastOptions={{
          unstyled: true,
          classNames: {
            toast:
              "flex w-[260px] flex-col gap-0.5 rounded-md border border-border bg-surface-2 px-3 py-2 shadow-[0_8px_24px_-8px_rgba(0,0,0,0.6)]",
            title: "text-[12.5px] font-medium text-foreground",
            description: "text-[11.5px] text-muted-foreground",
          },
        }}
      />
    </QueryClientProvider>
  );
}
