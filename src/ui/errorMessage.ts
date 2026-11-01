import { t } from "../i18n";

export function errorMessage(code: string): string | null {
  switch (code) {
    case "network": return t("err.network_help");
    case "protocol": return t("err.protocol_help");
    case "empty_answer": return t("err.empty_help");
    case "response_missing": return t("err.missing_help");
    case "turn_failed": return t("err.failed_help");
    default: {
      const http = /^http_(\d{3})$/.exec(code);
      return http ? t("err.http_help", { status: http[1] }) : null;
    }
  }
}
