import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "./demo";
import { t } from "../i18n";

/** Keep the native menu in the same language as the window, including changes. */
export async function syncTrayLabels(): Promise<void> {
  if (!inTauri) return;
  try {
    await invoke("set_tray_labels", { open: t("tray.open"), quit: t("common.close") });
  } catch (error) {
    console.error("could not update tray labels", error);
  }
}
