import { Monitor, Moon, Sun } from "lucide-react";
import { type ThemePreference, useTheme } from "@/components/theme-provider";
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
      className="flex items-center gap-0.5 rounded-md border border-border bg-muted/60 p-0.5"
      role="radiogroup"
      onKeyDown={(event) => {
        const index = themeOptions.findIndex(
          (option) => option.value === preference,
        );
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? 2
              : ["ArrowRight", "ArrowDown"].includes(event.key)
                ? (index + 1) % 3
                : ["ArrowLeft", "ArrowUp"].includes(event.key)
                  ? (index + 2) % 3
                  : null;
        if (next === null) return;
        event.preventDefault();
        setPreference(themeOptions[next].value);
        const buttons =
          event.currentTarget.querySelectorAll<HTMLButtonElement>("button");
        buttons[next]?.focus();
      }}
    >
      {themeOptions.map(({ value, label, Icon }) => (
        <Button
          aria-checked={preference === value}
          aria-label={label}
          title={label}
          tabIndex={preference === value ? 0 : -1}
          className="h-7 rounded px-2 text-xs"
          key={value}
          onClick={() => setPreference(value)}
          role="radio"
          size="sm"
          variant={preference === value ? "secondary" : "ghost"}
        >
          <Icon aria-hidden="true" className="size-3.5" />
          <span className="max-[760px]:hidden">{label}</span>
        </Button>
      ))}
    </div>
  );
}
