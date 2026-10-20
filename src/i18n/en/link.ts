/**
 * English strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Connect to a Yaca server",
  "link.intro": "Yaca runs on your server, and this app is a window onto it. Enter the address and the token the install script gave you.",
  "link.address": "Server address",
  "link.address_hint": "For example, yaca.example.com",
  "link.token": "Token",
  "link.token_hint": "Printed by yaca-server token add.",
  "link.connect": "Connect",
  "link.connecting": "Connecting…",
  "link.local": "Work on this computer",
  "link.err_unreachable": "The server does not answer. Check the address and the network.",
  "link.err_unauthorized": "The server did not accept the token.",
  "link.err_bad_address": "That does not look like a server address.",
  "link.err_replaced": "Another device connected to the server and took the chats. Connect again to bring them back here.",
  "link.err_revoked": "This device's token has been revoked.",
  "link.lost": "Lost the connection to the server. Reconnecting…",
  "link.settings_nav": "Connection",
  "link.settings_title": "Connection",
  "link.settings_this_pc": "Work happens on this computer.",
  "link.settings_server": "Work happens on the server {name}.",
  "link.settings_to_server": "Connect to a server",
  "link.settings_to_local": "Go back to this computer",
  "link.settings_disconnect": "Disconnect from the server",
  "link.settings_reload_note": "The window reloads. Chats on the server keep running.",
  "link.pick_up": "Up",
  "link.pick_home": "Home",
  "link.pick_choose": "Choose this folder",
  "link.pick_new": "New folder",
  "link.pick_new_name": "Folder name",
  "link.pick_create": "Create",
  "link.pick_cancel": "Cancel",
  "link.pick_empty": "No folders in here",
  "link.pick_hidden": "Hidden folders",
  "link.pick_broad": "This looks like a home or root folder, not a project. The agent would work on everything in it, and snapshots for rolling back will not work.",
  "link.pick_loading": "Reading the folder…",
} satisfies Record<keyof typeof source, string>;
