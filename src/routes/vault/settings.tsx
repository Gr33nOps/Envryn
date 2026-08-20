import * as React from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
  Button,
  PageHeader,
  Panel,
  SectionLabel,
  Select,
  SettingsRow,
  Switch,
} from "@/components/envryn/ui";

export const Route = createFileRoute("/vault/settings")({
  component: Settings,
});

function Group({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-1.5">
      <SectionLabel>{label}</SectionLabel>
      <Panel>{children}</Panel>
    </section>
  );
}

function Settings() {
  const [requireAuth, setRequireAuth] = React.useState(true);
  const [lockWithWindows, setLockWithWindows] = React.useState(true);
  const [discovery, setDiscovery] = React.useState(true);

  return (
    <>
      <PageHeader title="Settings" />
      <div className="max-w-[620px] space-y-5 px-5 pb-8">
        <Group label="Security">
          <SettingsRow
            label="Auto lock"
            control={
              <Select defaultValue="5" className="w-[130px]">
                <option value="1">1 minute</option>
                <option value="5">5 minutes</option>
                <option value="15">15 minutes</option>
                <option value="0">Never</option>
              </Select>
            }
          />
          <SettingsRow
            label="Require authentication to reveal secrets"
            control={<Switch checked={requireAuth} onCheckedChange={setRequireAuth} />}
          />
          <SettingsRow
            label="Clipboard clearing"
            control={
              <Select defaultValue="30" className="w-[130px]">
                <option value="10">10 seconds</option>
                <option value="30">30 seconds</option>
                <option value="60">60 seconds</option>
              </Select>
            }
          />
          <SettingsRow
            label="Lock when Windows locks"
            control={
              <Switch
                checked={lockWithWindows}
                onCheckedChange={setLockWithWindows}
              />
            }
          />
        </Group>

        <Group label="Appearance">
          <SettingsRow
            label="Theme"
            control={
              <Select defaultValue="dark" className="w-[130px]">
                <option value="dark">Dark</option>
                <option value="system" disabled>
                  System (soon)
                </option>
                <option value="light" disabled>
                  Light (soon)
                </option>
              </Select>
            }
          />
        </Group>

        <Group label="Sync">
          <SettingsRow
            label="Local device discovery"
            description="Allow trusted devices to find this computer on your network."
            control={<Switch checked={discovery} onCheckedChange={setDiscovery} />}
          />
        </Group>

        <Group label="Data">
          <SettingsRow
            label="Backup"
            control={
              <Link to="/vault/backup">
                <Button size="sm">Open</Button>
              </Link>
            }
          />
          <SettingsRow label="Manage vault" control={<Button size="sm">Open</Button>} />
        </Group>

        <Group label="Keyboard">
          <SettingsRow
            label="Search"
            control={<span className="kbd">Ctrl K</span>}
          />
          <SettingsRow
            label="Add secret"
            control={<span className="kbd">Ctrl N</span>}
          />
          <SettingsRow
            label="Lock vault"
            control={<span className="kbd">Ctrl L</span>}
          />
        </Group>

        <Group label="About">
          <SettingsRow label="Envryn" control={<span className="text-[12px] text-muted-foreground">Version 0.1.0</span>} />
          <SettingsRow
            label="Open-source licenses"
            control={<Button size="sm">View</Button>}
          />
          <SettingsRow
            label="Security documentation"
            control={<Button size="sm">View</Button>}
          />
        </Group>
      </div>
    </>
  );
}
