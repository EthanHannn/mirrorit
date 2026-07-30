import { Monitor, Moon, Sun } from "lucide-react";
import {
  type ThemePreference,
  useTheme,
} from "@/components/theme-provider";
import { Button } from "@/components/ui/button";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  Icon: typeof Sun;
}> = [
  { value: "light", label: "浅色", Icon: Sun },
  { value: "dark", label: "深色", Icon: Moon },
  { value: "system", label: "系统", Icon: Monitor },
];

export function ThemeToggle() {
  const { preference, setPreference } = useTheme();

  return (
    <div
      aria-label="主题"
      className="flex items-center rounded-lg border border-border bg-card p-0.5"
      role="radiogroup"
    >
      {themeOptions.map(({ value, label, Icon }) => (
        <Button
          aria-checked={preference === value}
          className="h-7 px-2 text-xs"
          key={value}
          onClick={() => setPreference(value)}
          role="radio"
          size="sm"
          variant={preference === value ? "secondary" : "ghost"}
        >
          <Icon aria-hidden="true" className="size-3.5" />
          {label}
        </Button>
      ))}
    </div>
  );
}
