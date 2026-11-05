/**
 * The update as the screens see it: what the last answer was, whether a check
 * is running now, why the last one failed, and how far an install has got.
 *
 * One answer per application, not per screen. The shell keeps it and
 * broadcasts it (`update`), so the sidebar card and the About screen read the
 * same fact, whichever of them started the check or the download.
 */

import { useCallback, useEffect, useState } from "react";

import {
  canInstallUpdates,
  checkForUpdates,
  installAppUpdate,
  onUpdate,
  onUpdateInstall,
  updateInstallStatus,
  updateStatus,
  type InstallPhase,
  type ReleaseInfo,
  type UpdateCheck,
  type UpdateInstall,
} from "../core/client";

export type UpdateView = {
  /** `null` until a check has finished. */
  check: UpdateCheck | null;
  checking: boolean;
  /** A code from the shell (`offline`, `rate_limited`, `status:502`, ...). */
  error: string | null;
  /** When the answer on screen arrived, in this session. */
  checkedAt: Date | null;
  run: () => void;
  /**
   * Downloads the release the last check found, verifies its signature and
   * installs it. `force` is the second press of a person told that a turn is
   * running.
   */
  install: (force?: boolean) => void;
  /** `false` in the stand, where there is no shell and nothing to install into. */
  canInstall: boolean;
  installPhase: InstallPhase;
  /** Whole percents of the download, or `null` while its size is unknown. */
  percent: number | null;
  /** Why the last install stopped, as a code; `null` while none did. */
  installError: string | null;
};

/** Nothing is downloading: what a fresh window starts from. */
const IDLE: UpdateInstall = { phase: "idle", received: 0, total: null, error: null };

/** What the sidebar card shows: the release, and the install of it if any. */
export type UpdateNotice = {
  release: ReleaseInfo;
  phase: InstallPhase;
  /** Whole percents of the download, or `null` while its size is unknown. */
  percent: number | null;
  /** A code from the shell: `busy`, `signature`, `install_failed`, ... */
  error: string | null;
  /** Whether this window installs at all, or opens the release page instead. */
  canInstall: boolean;
};

export function useUpdate(): UpdateView {
  const [check, setCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const [progress, setProgress] = useState<UpdateInstall>(IDLE);

  useEffect(() => {
    let live = true;
    let off: (() => void) | null = null;
    let offInstall: (() => void) | null = null;

    void updateStatus().then(
      (answer) => {
        if (live && answer !== null) setCheck((now) => now ?? answer);
      },
      () => undefined,
    );
    // A window that was reloaded in the middle of a download must not come back
    // offering the release again while the file is still arriving.
    void updateInstallStatus().then(
      (answer) => {
        if (live) setProgress((now) => (now.phase === "idle" ? answer : now));
      },
      () => undefined,
    );
    void onUpdate((answer) => {
      if (!live) return;
      setCheck(answer);
      setError(null);
      setCheckedAt(new Date());
    }).then((unlisten) => {
      if (live) off = unlisten;
      else unlisten();
    });
    void onUpdateInstall((answer) => {
      if (live) setProgress(answer);
    }).then((unlisten) => {
      if (live) offInstall = unlisten;
      else unlisten();
    });

    return () => {
      live = false;
      off?.();
      offInstall?.();
    };
  }, []);

  const run = useCallback(() => {
    setChecking(true);
    setError(null);
    checkForUpdates().then(
      (answer) => {
        setCheck(answer);
        setCheckedAt(new Date());
        setChecking(false);
      },
      (e: unknown) => {
        setError(typeof e === "string" ? e : "offline");
        setChecking(false);
      },
    );
  }, []);

  const install = useCallback((force = false) => {
    void installAppUpdate(force).catch((e: unknown) => {
      // The shell publishes its refusals as well; this keeps the words on
      // screen even if that message crossed the window's reload.
      setProgress((now) => ({
        ...now,
        phase: "failed",
        error: typeof e === "string" ? e : "install_failed",
      }));
    });
  }, []);

  return {
    check,
    checking,
    error,
    checkedAt,
    run,
    install,
    canInstall: canInstallUpdates,
    installPhase: progress.phase,
    percent: percentOf(progress.received, progress.total),
    installError: progress.error,
  };
}

/** Whole percents of a download, or `null` while the size is not known yet. */
export function percentOf(received: number, total: number | null): number | null {
  if (total === null || total <= 0) return null;
  const percent = Math.round((received / total) * 100);
  return Math.min(100, Math.max(0, percent));
}

/** The words for a failure code of the check, as a dictionary key. */
export function updateErrorKey(code: string) {
  if (code === "offline") return "about.update_offline" as const;
  if (code === "rate_limited") return "about.update_rate_limited" as const;
  return "about.update_failed" as const;
}

/** The words for a refusal or a failure of the install, as a dictionary key. */
export function installErrorKey(code: string) {
  switch (code) {
    case "offline":
      return "about.update_offline" as const;
    case "signature":
      return "about.update_signature" as const;
    case "no_artifact":
      return "about.update_no_artifact" as const;
    case "busy":
      return "about.update_busy" as const;
    case "nothing":
      return "about.update_failed" as const;
    case "dev_build":
      return "about.update_dev" as const;
    default:
      return "about.update_install_error" as const;
  }
}
