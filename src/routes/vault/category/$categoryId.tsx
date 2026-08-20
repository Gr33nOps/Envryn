import { createFileRoute, notFound } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { categories, secrets } from "@/lib/envryn-data";
import { SecretList } from "@/components/envryn/SecretList";
import { useVaultUI } from "@/components/envryn/vault-context";
import { Button, EmptyState, PageHeader } from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/category/$categoryId")({
  loader: ({ params }) => {
    const category = categories[params.categoryId as keyof typeof categories];
    if (!category) throw notFound();
    return category;
  },
  component: CategoryView,
});

function CategoryView() {
  const category = Route.useLoaderData();
  const { openAdd } = useVaultUI();
  const items = secrets.filter((s) => category.types.includes(s.type));

  return (
    <>
      <PageHeader
        title={category.label}
        subtitle={`${items.length} secrets`}
        actions={
          <Button variant="primary" onClick={() => openAdd()}>
            <Plus />
            Add Secret
          </Button>
        }
      />
      <div className="px-5 pb-5">
        {items.length === 0 ? (
          <EmptyState
            title={`Nothing in ${category.label}`}
            body="Secrets of this type will appear here."
          />
        ) : (
          <SecretList items={items} columns={["project", "environment", "updated"]} />
        )}
      </div>
    </>
  );
}
