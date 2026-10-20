import {
  createContext,
  useContext,
  useEffect,
  useState,
  type PropsWithChildren,
} from "react";
import { russian } from "./translations/ru";
export type Language = "ru" | "en";
const KEY = "ak820-language";
let language: Language = (() => {
  try {
    const saved = localStorage.getItem(KEY);
    return saved === "en" || saved === "ru"
      ? saved
      : navigator.language.startsWith("ru")
        ? "ru"
        : "en";
  } catch {
    return "en";
  }
})();
export function t(english: string, ru?: string): string {
  if (language === "en") return english;
  if (english.startsWith("Error: ")) return `Ошибка: ${t(english.slice(7))}`;
  return ru ?? russian[english] ?? english;
}
const Context = createContext({
  language,
  setLanguage: (_value: Language) => {},
});
export function LanguageProvider({ children }: PropsWithChildren) {
  const [value, setValue] = useState(language);
  const setLanguage = (next: Language) => {
    language = next;
    try {
      localStorage.setItem(KEY, next);
    } catch {
      /* unavailable storage */
    }
    setValue(next);
  };
  useEffect(() => {
    document.documentElement.lang = value;
  }, [value]);
  return (
    <Context.Provider value={{ language: value, setLanguage }}>
      {children}
    </Context.Provider>
  );
}
export function useI18n() {
  return { ...useContext(Context), t };
}
