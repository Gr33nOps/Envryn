import { toast } from "sonner";
import type { Secret } from "./envryn-data";

export async function copySecret(secret?: Pick<Secret, "value">) {
  if (secret?.value && typeof navigator !== "undefined") {
    await navigator.clipboard?.writeText(secret.value);
  }
  toast("Secret copied", { description: "Clipboard clears in 30 seconds." });
}
