/**
 * Getting a connection before the window draws anything else.
 *
 * Against a server nothing works until the socket is open and has accepted the
 * token: the preferences that pick the language and the theme are on the
 * server too. So the page starts here, with what a person needs to get
 * connected, and hands over to the application only once there is a host to
 * talk to. Locally (and in the stand) there is nothing to wait for.
 */

import type { Root } from "react-dom/client";

import { isRemote, savedRemote } from "../../core/link";
import { link, type LinkFault } from "../../core/remote";
import { ConnectScreen } from "./ConnectScreen";

/** Resolves once calls can be made. May render the connect screen into `root` first. */
export async function openLink(root: Root): Promise<void> {
  if (!isRemote()) return;

  let fault: LinkFault | null = null;
  const remote = savedRemote();
  if (remote !== null) {
    try {
      await link.connect(remote);
      return;
    } catch (e) {
      fault = e instanceof Error && "fault" in e ? (e as { fault: LinkFault }).fault : "unreachable";
    }
  }

  await new Promise<void>((done) => {
    root.render(<ConnectScreen first={fault} onConnected={done} />);
  });
}
