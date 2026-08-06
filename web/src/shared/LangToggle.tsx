import { Languages } from "lucide-react";

import { useLanguage } from "./language";
import { Button } from "./ui/button";

/**
 * Toggles the site language between Korean (ko) and English (en).
 * Must be rendered inside a LanguageProvider (Shell/HubLobby).
 */
export function LangToggle() {
  const { lang, setLang } = useLanguage();
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="언어 전환 / Switch language"
      onClick={() => setLang(lang === "ko" ? "en" : "ko")}
    >
      <Languages />
      {lang === "ko" ? "EN" : "KO"}
    </Button>
  );
}