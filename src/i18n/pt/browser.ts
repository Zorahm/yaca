/**
 * Portuguese strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "Navegador",
  "browser.back": "Voltar",
  "browser.forward": "Avançar",
  "browser.reload": "Recarregar",
  "browser.stop": "Parar o carregamento",
  "browser.address": "Endereço ou localhost:porta",
  "browser.external": "Abrir no navegador do sistema",
  "browser.devtools": "Ferramentas de desenvolvedor",
  "browser.empty_title": "Nada aberto ainda",
  "browser.empty_body": "A página que você ou o modelo abrir aparecerá aqui",
  "browser.size_model": "{size} · definido pelo modelo",
  "browser.reset_size": "Voltar ao tamanho da coluna",
  "browser.resize": "Redimensionar o navegador",
  "browser.bad_address": "Isso não parece o endereço de uma página",
  "browser.open_failed": "A página não abriu",
  "browser.busy": "O modelo está usando o navegador",
} satisfies Record<keyof typeof source, string>;
