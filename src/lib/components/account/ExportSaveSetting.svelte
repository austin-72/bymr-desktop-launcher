<script lang="ts">
  import { fade } from "svelte/transition";
  import Check from "@lucide/svelte/icons/check";
  import Copy from "@lucide/svelte/icons/copy";
  import Download from "@lucide/svelte/icons/download";
  import FileDown from "@lucide/svelte/icons/file-down";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import { invoke } from "@tauri-apps/api/core";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { toErrorMessage } from "#lib/api/errors.js";
  import { exportSave } from "#lib/api/player.js";
  import SettingRow from "#lib/components/account/SettingRow.svelte";
  import Button from "#lib/components/ui/Button.svelte";
  import { DURATION_MS } from "#lib/motion.js";
  import type { AccountDetails } from "#lib/schemas.js";
  import { logs } from "#lib/state/logs.svelte.js";

  interface Props {
    details: AccountDetails;
  }

  let { details }: Props = $props();

  const COPIED_MS = 2000;

  let exporting = $state(false);
  let code = $state("");
  let savedPath = $state("");
  let copied = $state(false);
  let error = $state("");

  const savedName = $derived(savedPath.split(/[\\/]/).pop() ?? "");

  /**
   * The file name for today's export, e.g. "bym-save-Kyle-2026-11-24". The launcher adds the extension.
   *
   * @returns {string} The name without an extension.
   */
  const fileName = () => {
    const date = new Date();
    const day = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

    return `bym-save-${details.username}-${day}`;
  };

  /**
   * Fetches a signed save code from the server and writes it to the Downloads folder.
   *
   * @returns {Promise<void>} Resolves once the file is saved or the export failed.
   */
  const exportToFile = async () => {
    exporting = true;
    code = "";
    savedPath = "";
    copied = false;
    error = "";

    try {
      code = (await exportSave()).code;
      savedPath = await invoke<string>("save_export_file", { fileName: fileName(), contents: code });
      logs.success(`Exported your save to ${savedPath}`);
    } catch (failure) {
      error = toErrorMessage(failure);
      logs.error(`Could not export your save: ${error}`);
    } finally {
      exporting = false;
    }
  };

  /**
   * Copies the save code, ready to paste on a custom server's register screen.
   *
   * @returns {Promise<void>} Resolves once the code is copied or that failed.
   */
  const copyCode = async () => {
    try {
      await navigator.clipboard.writeText(code);
      copied = true;
      setTimeout(() => (copied = false), COPIED_MS);
    } catch (failure) {
      error = `Could not copy the code. Open ${savedName} and copy everything in it instead.`;
      logs.error(`Could not copy the save code: ${toErrorMessage(failure)}`);
    }
  };

  /**
   * Opens the folder the export was saved to, with the file selected.
   *
   * @returns {Promise<void>} Resolves once the folder is opened or that failed.
   */
  const showInFolder = async () => {
    try {
      await revealItemInDir(savedPath);
    } catch (failure) {
      logs.error(`Could not open the folder: ${toErrorMessage(failure)}`);
    }
  };
</script>

<SettingRow
  title="Export save"
  description="Makes a save code from your main and inferno yards, signed by this server. Paste it when registering on a custom server to keep your progress there. Your Map Room position, outposts and alliance aren't included."
>
  {#snippet icon()}<FileDown class="size-[18px]" />{/snippet}
  <Button variant="outline" class="w-fit" disabled={exporting} onclick={exportToFile}>
    <Download />{exporting ? "Exporting..." : "Export save"}
  </Button>
  {#if error}
    <p class="text-[12.5px] text-destructive">{error}</p>
  {/if}
  {#if savedPath}
    <div class="grid gap-1 text-[12.5px]" in:fade={{ duration: DURATION_MS }}>
      <p class="flex min-w-0 items-center gap-1.5 text-success" title={savedPath}>
        <Check class="size-3.5 shrink-0" /><span class="truncate">Saved as {savedName}</span>
      </p>
      <div class="flex flex-wrap items-center gap-x-4">
        <Button variant="link" size="sm" class="h-auto px-0 text-[12.5px]" onclick={copyCode}>
          {#if copied}<Check class="size-3.5" />Copied{:else}<Copy class="size-3.5" />Copy code{/if}
        </Button>
        <Button variant="link" size="sm" class="h-auto px-0 text-[12.5px]" onclick={showInFolder}>
          <FolderOpen class="size-3.5" />Show in folder
        </Button>
      </div>
    </div>
  {:else if !error}
    <p class="text-[12.5px] text-muted-foreground">Saved to your Downloads folder. Up to 10 exports an hour.</p>
  {/if}
</SettingRow>
